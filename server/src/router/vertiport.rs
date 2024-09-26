//! Vertiport-related utilities

use super::flight_plan::*;
use super::schedule::*;
use super::vehicle::*;
use super::{best_path, BestPathError, BestPathRequest};
use crate::grpc::client::GrpcClients;
use lib_common::time::Duration;
use std::cmp::{max, min};
use std::collections::HashMap;
use std::str::FromStr;
use svc_gis_client_grpc::prelude::gis::*;
use svc_storage_client_grpc::prelude::*;

/// Chop up larger timeslots into smaller durations to avoid temporary no-fly zones
const MAX_DURATION_TIMESLOT_MINUTES: i64 = 30;

/// Error type for vertiport-related errors
#[derive(Debug, Copy, Clone)]
pub enum VertiportError {
    /// Error communicating with a client
    ClientError,

    /// Invalid data
    Data,

    /// No vertipads found
    NoVertipads,

    /// No schedule found
    NoSchedule,

    /// Invalid schedule
    Schedule,

    /// Internal error
    Internal,
}

impl std::fmt::Display for VertiportError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            VertiportError::ClientError => write!(f, "Client error"),
            VertiportError::Data => write!(f, "Invalid data"),
            VertiportError::NoVertipads => write!(f, "No vertipads"),
            VertiportError::NoSchedule => write!(f, "No schedule"),
            VertiportError::Schedule => write!(f, "Invalid schedule"),
            VertiportError::Internal => write!(f, "Internal error"),
        }
    }
}

/// Argument for getting vertipads, either by parent vertiport ID or by vertipad IDs
pub enum GetVertipadsArg {
    VertiportId(String),
    VertipadIds(Vec<String>),
}

/// Gets all vertipads for a vertiport
pub async fn get_vertipads(
    clients: &GrpcClients,
    arg: GetVertipadsArg,
) -> Result<Vec<String>, VertiportError> {
    let mut filter = AdvancedSearchFilter::search_is_null("deleted_at".to_owned());
    // TODO(R5): factor in enabled vs disabled

    match arg {
        GetVertipadsArg::VertiportId(vertiport_id) => {
            filter = filter.and_equals("vertiport_id".to_string(), vertiport_id);
        }
        GetVertipadsArg::VertipadIds(ids) => {
            filter = filter.and_in("vertipad_id".to_string(), ids);
        }
    }

    router_info!("proposed filter: {:?}", filter.clone());

    let response = clients
        .storage
        .vertipad
        .search(filter)
        .await
        .map_err(|e| {
            router_error!("Failed to get vertipads: {e}");
            VertiportError::NoVertipads
        })?
        .into_inner()
        .list
        .into_iter()
        // in R3 the search filter is not working, do an extra filter here
        .filter_map(|vp| {
            let Some(data) = vp.data else {
                return None;
            };

            if !data.enabled {
                return None;
            }

            Some(vp.id)
        })
        .collect::<Vec<String>>();

    router_info!("response: {:?}", response);
    Ok(response)
}

/// Get pairs of timeslots where a flight can leave within the origin timeslot
///  and land within the target timeslot
#[allow(clippy::too_many_arguments)]
pub async fn get_timeslot_pairs(
    origin_vertiport_id: &str,
    origin_vertipad_id: Option<&str>,
    target_vertiport_id: &str,
    target_vertipad_id: Option<&str>,
    origin_time_block: &Duration,
    target_time_block: &Duration,
    timeslot: &Timeslot,
    existing_flight_plans: &[FlightPlanSchedule],
    clients: &GrpcClients,
) -> Result<Vec<TimeslotPair>, VertiportError> {
    let origin_timeslots = get_available_timeslots(
        origin_vertiport_id,
        origin_vertipad_id,
        existing_flight_plans,
        timeslot,
        origin_time_block,
        clients,
    )
    .await?;

    let target_timeslots = get_available_timeslots(
        target_vertiport_id,
        target_vertipad_id,
        existing_flight_plans,
        timeslot,
        target_time_block,
        clients,
    )
    .await?;

    get_vertipad_timeslot_pairs(
        origin_vertiport_id,
        target_vertiport_id,
        origin_timeslots,
        target_timeslots,
        clients,
    )
    .await
}

/// Return a map of vertipad ids to available timeslots for that vertipad
///
/// TODO(R5): This will be replaced with a call to svc-storage vertipad_timeslots to
///  return a list of available timeslots for each vertipad, so we don't
///  need to rebuild each pad's schedule from flight plans each time
pub async fn get_available_timeslots(
    vertiport_id: &str,
    vertipad_id: Option<&str>,
    existing_flight_plans: &[FlightPlanSchedule],
    timeslot: &Timeslot,
    minimum_duration: &Duration,
    clients: &GrpcClients,
) -> Result<HashMap<String, Vec<Timeslot>>, VertiportError> {
    // Get vertiport schedule
    let calendar = get_vertiport_calendar(vertiport_id, clients).await?;

    // TODO(R5): Use each vertipad's calendar
    let base_timeslots = calendar
        .to_timeslots(&timeslot.time_start(), &timeslot.time_end())
        .map_err(|e| {
            router_error!("Could not convert calendar to timeslots: {e}");
            VertiportError::Internal
        })?;

    router_debug!("base_timeslots: {:?}", base_timeslots);

    // TODO(R5): This is currently hardcoded, get the duration of the timeslot
    // try min and max both the necessary landing time
    let max_duration = Duration::try_minutes(MAX_DURATION_TIMESLOT_MINUTES).ok_or_else(|| {
        router_error!("error creating time delta.");
        VertiportError::Internal
    })?;

    let filter = match vertipad_id {
        Some(id) => GetVertipadsArg::VertipadIds(vec![id.to_string()]),
        None => GetVertipadsArg::VertiportId(vertiport_id.to_string()),
    };

    // Prepare a list of slots for each vertipad
    // For now, each vertipad shares the same schedule as the vertiport itself
    let mut timeslots = get_vertipads(clients, filter)
        .await?
        .into_iter()
        .map(|id| (id, base_timeslots.clone()))
        .collect::<HashMap<String, Vec<Timeslot>>>();

    // Get occupied slots
    // TODO(R5): This will be replaced with a call to svc-storage vertipad_timeslots to
    //  return a list of occupied timeslots for each vertipad, so we don't
    //  need to rebuild each pad's schedule from flight plans each time
    let occupied_slots = build_timeslots_from_flight_plans(vertiport_id, existing_flight_plans)?;

    router_debug!("(get_available_timeslots): vertiport: {:?}", vertiport_id);
    router_debug!("(get_available_timeslots): vertipads {:?}", timeslots);
    router_debug!(
        "(get_available_timeslots): occupied {:?}",
        occupied_slots
            .iter()
            .map(|(id, _)| id)
            .collect::<Vec<&String>>()
    );

    // For each occupied slot, remove it from the list of available slots
    for (vertipad_id, occupied_slot) in occupied_slots.iter() {
        let Some(vertipad_slots) = timeslots.get_mut(vertipad_id) else {
            router_error!(
                "Vertipad {} (from a flight plan) not found in list of vertipads from storage.",
                vertipad_id
            );
            continue;
        };

        *vertipad_slots = vertipad_slots
            .iter_mut()
            // Subtract the occupation slot from the available slots
            .flat_map(|slot| *slot - *occupied_slot)
            // Split any slots that are too long. A short temporary no-fly zone overlapping
            //  any part of the timeslot will invalidate the entire timeslot, so we split it
            //  into smaller timeslots to avoid this.
            .flat_map(|slot| slot.split(minimum_duration, &max_duration))
            .collect::<Vec<Timeslot>>();
    }

    Ok(timeslots)
}

/// Gets vertiport schedule from storage and converts it to a Calendar object.
async fn get_vertiport_calendar(
    vertiport_id: &str,
    clients: &GrpcClients,
) -> Result<Calendar, VertiportError> {
    let vertiport_schedule = clients
        .storage
        .vertiport
        .get_by_id(Id {
            id: vertiport_id.to_string(),
        })
        .await
        .map_err(|e| {
            router_error!("Could not retrieve data for vertiport {vertiport_id}: {e}");
            VertiportError::ClientError
        })?
        .into_inner()
        .data
        .ok_or_else(|| {
            router_error!("Date invalid for vertiport {}.", vertiport_id);
            VertiportError::Data
        })?
        .schedule
        .ok_or_else(|| {
            router_error!("No schedule for vertiport {}.", vertiport_id);
            VertiportError::NoSchedule
        })?;

    Calendar::from_str(&vertiport_schedule).map_err(|e| {
        router_error!("Schedule invalid for vertiport {vertiport_id}; {e}.");
        VertiportError::Schedule
    })
}

/// Gets all occupied vertipad time slots given flight plans.
///  If `invert` is true, returns all unoccupied time slots.
///
/// TODO(R5): Remove in favor of read from storage vertipad_timeslot table
///  where the duration of the timeslot is stored
fn build_timeslots_from_flight_plans(
    vertiport_id: &str,
    flight_plans: &[FlightPlanSchedule],
) -> Result<Vec<(String, Timeslot)>, VertiportError> {
    // TODO(R5): This is currently hardcoded, get the duration of the timeslot
    //  directly from the vertipad_timeslot object
    let required_loading_time =
        Duration::try_seconds(crate::grpc::api::query_flight::LOADING_AND_TAKEOFF_TIME_SECONDS)
            .ok_or_else(|| {
                router_error!("error creating time delta.");
                VertiportError::Internal
            })?;

    let required_unloading_time =
        Duration::try_seconds(crate::grpc::api::query_flight::LANDING_AND_UNLOADING_TIME_SECONDS)
            .ok_or_else(|| {
            router_error!("error creating time delta.");
            VertiportError::Internal
        })?;

    let results = flight_plans
        .iter()
        .filter_map(|fp| {
            if *vertiport_id == fp.origin_vertiport_id {
                // TODO(R5): duration should be retrieved from flight plan object
                //  instead of being hardcoded
                let timeslot = Timeslot::new(
                    fp.origin_timeslot_start,
                    fp.origin_timeslot_start + required_loading_time,
                );

                match timeslot {
                    Ok(timeslot) => Some((fp.origin_vertipad_id.clone(), timeslot)),
                    Err(_) => None,
                }
            } else if *vertiport_id == fp.target_vertiport_id {
                // TODO(R5): duration should be retrieved from flight plan object
                //  instead of being hardcoded
                let timeslot = Timeslot::new(
                    fp.target_timeslot_start,
                    fp.target_timeslot_start + required_unloading_time,
                );

                match timeslot {
                    Ok(timeslot) => Some((fp.target_vertipad_id.clone(), timeslot)),
                    Err(_) => None,
                }
            } else {
                None
            }
        })
        .collect::<Vec<(String, Timeslot)>>();

    Ok(results)
}

/// Gets all available timeslot pairs and a path for each pair
#[derive(Debug, Clone)]
pub struct TimeslotPair {
    pub origin_vertiport_id: String,
    pub origin_vertipad_id: String,
    pub origin_timeslot: Timeslot,
    pub target_vertiport_id: String,
    pub target_vertipad_id: String,
    pub target_timeslot: Timeslot,
    pub waypoints: Vec<PointZ>,
    pub distance_meters: f64,
}

impl From<TimeslotPair> for flight_plan::Data {
    fn from(val: TimeslotPair) -> Self {
        let points = val
            .waypoints
            .iter()
            .map(|p| GeoPointZ {
                y: p.latitude,
                x: p.longitude,
                z: p.altitude_meters as f64,
            })
            .collect();

        let waypoints = Some(GeoLineStringZ { points });
        flight_plan::Data {
            origin_vertiport_id: val.origin_vertiport_id,
            origin_vertipad_id: val.origin_vertipad_id,
            origin_timeslot_start: Some(val.origin_timeslot.time_start().into()),
            origin_timeslot_end: Some(val.origin_timeslot.time_end().into()),
            target_vertiport_id: val.target_vertiport_id,
            target_vertipad_id: val.target_vertipad_id,
            target_timeslot_start: Some(val.target_timeslot.time_start().into()),
            target_timeslot_end: Some(val.target_timeslot.time_end().into()),
            waypoints,
            ..Default::default()
        }
    }
}

/// Attempts to find a pairing of origin and target pad
///  timeslots wherein a flight could occur.
pub async fn get_vertipad_timeslot_pairs(
    origin_vertiport_id: &str,
    target_vertiport_id: &str,
    origin_vertipads: HashMap<String, Vec<Timeslot>>,
    target_vertipads: HashMap<String, Vec<Timeslot>>,
    clients: &GrpcClients,
) -> Result<Vec<TimeslotPair>, VertiportError> {
    let mut pairs = vec![];
    let mut best_path_request = BestPathRequest {
        origin_identifier: origin_vertiport_id.to_string(),
        target_identifier: target_vertiport_id.to_string(),
        origin_type: NodeType::Vertiport as i32,
        target_type: NodeType::Vertiport as i32,
        time_start: None,
        time_end: None,
        limit: 5,
    };

    let mut origin_timeslots = origin_vertipads
        .into_iter()
        .flat_map(|(id, slots)| slots.into_iter().map(move |slot| (id.clone(), slot)))
        .collect::<Vec<(String, Timeslot)>>();
    origin_timeslots.sort_by(|a, b| a.1.time_start().cmp(&b.1.time_start()));

    let mut target_timeslots = target_vertipads
        .into_iter()
        .flat_map(|(id, slots)| slots.into_iter().map(move |slot| (id.clone(), slot)))
        .collect::<Vec<(String, Timeslot)>>();
    target_timeslots.sort_by(|a, b| a.1.time_start().cmp(&b.1.time_start()));

    // Iterate through origin pads and their schedules
    for (origin_vertipad_id, ots) in origin_timeslots.iter_mut() {
        // Iterate through target pads and their schedules
        'target: for (target_vertipad_id, tts) in target_timeslots.iter_mut() {
            // no timeslot overlap possible
            //                    | origin timeslot |
            // | target timeslot |
            if ots.time_start() >= tts.time_end() {
                continue;
            }

            // Temporary no-fly zones make checking the same route
            //  multiple times necessary for different timeslots
            best_path_request.time_start = Some(ots.time_start().into());
            best_path_request.time_end = Some(tts.time_end().into());

            let mut paths = match best_path(&best_path_request, clients).await {
                Ok(paths) => paths,
                Err(BestPathError::NoPathFound) => {
                    // no path found, perhaps temporary no-fly zone
                    //  is blocking journeys from this depart timeslot
                    // Break out and try the next depart timeslot
                    router_debug!(
                        "No path found from vertiport {}
                            to vertiport {} (from {} to {}).",
                        origin_vertiport_id,
                        target_vertiport_id,
                        ots.time_start(),
                        tts.time_end()
                    );

                    break 'target;
                }
                Err(BestPathError::ClientError) => {
                    // exit immediately if svc-gis is down, don't allow new flights
                    router_error!("Could not determine path - client error.");
                    return Err(VertiportError::ClientError);
                }
            };

            // For now only get the first path
            let (path, distance_meters) = paths.remove(0);
            let waypoints = path
                .into_iter()
                .filter_map(|node| {
                    if node.node_type != NodeType::Waypoint as i32 {
                        return None;
                    }

                    node.geom.map(|geom| PointZ {
                        latitude: geom.latitude,
                        longitude: geom.longitude,
                        altitude_meters: geom.altitude_meters,
                    })
                })
                .collect::<Vec<PointZ>>();

            let estimated_duration_s =
                estimate_flight_time_seconds(&distance_meters).map_err(|e| {
                    router_error!("Could not estimate flight time: {e}");
                    VertiportError::Internal
                })?;

            // Since both schedules are sorted, we can break early once
            //  origin end time + flight time is less than the target timeslot's start time
            //  and not look at the other timeslots for that pad
            // | origin timeslot |
            //                      ---->x
            //                                | target timeslot 1 | target timeslot 2 |
            // (the next target timeslot start to be checked would be even further away)
            if ots.time_end() + estimated_duration_s < tts.time_start() {
                break;
            }

            //
            // |     ots              |          (depart timeslot)
            //       ----->        ----->        (flight time)
            //            |      tts     |       (target timeslot)
            //       | actual ots  |             (actual depart timeslot)
            //
            // The actual origin_timeslot is the timeslot within which origin
            //  will result in landing in the target timeslot.
            let origin_timeslot = Timeslot::new(
                max(ots.time_start(), tts.time_start() - estimated_duration_s),
                min(ots.time_end(), tts.time_end() - estimated_duration_s),
            )
            .map_err(|e| {
                router_error!("Could not create timeslot: {e}");
                VertiportError::Internal
            })?;

            //
            //  |     ots     |             (depart timeslot)
            //   ----->       ----->        (flight time)
            //      |    tts            |   (target timeslot)
            //         | actual tts |
            // The actual target_timeslot is the timeslot within which target is possible
            //  given a origin from the actual depart timeslot.
            let target_timeslot = Timeslot::new(
                max(
                    tts.time_start(),
                    origin_timeslot.time_start() + estimated_duration_s,
                ),
                min(
                    tts.time_end(),
                    origin_timeslot.time_end() + estimated_duration_s,
                ),
            )
            .map_err(|e| {
                router_error!("Could not create timeslot: {e}");
                VertiportError::Internal
            })?;

            pairs.push(TimeslotPair {
                origin_vertiport_id: origin_vertiport_id.to_string(),
                origin_vertipad_id: origin_vertipad_id.clone(),
                origin_timeslot,
                target_vertiport_id: target_vertiport_id.to_string(),
                target_vertipad_id: target_vertipad_id.clone(),
                target_timeslot,
                waypoints, // leave off ground points
                distance_meters,
            });
        }
    }

    // Sort available options by shortest distance first
    pairs.sort_by(
        |a, b| match a.distance_meters.partial_cmp(&b.distance_meters) {
            Some(ord) => ord,
            None => {
                router_error!(
                    "Could not compare distances: {}, {}",
                    a.distance_meters,
                    b.distance_meters
                );

                std::cmp::Ordering::Equal
            }
        },
    );

    Ok(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::client::get_clients;
    use crate::router::vehicle::estimate_flight_time_seconds;
    use lib_common::time::{DateTime, Utc};
    use lib_common::uuid::Uuid;

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn ut_get_vertipad_pairs_no_overlap() {
        let origin_vertiport_id: String = Uuid::new_v4().to_string();
        let target_vertiport_id: String = Uuid::new_v4().to_string();
        let origin_vertipad_id: String = Uuid::new_v4().to_string();
        let target_vertipad_id: String = Uuid::new_v4().to_string();
        let clients = get_clients().await;

        //
        // CASE 1: No overlap, even leaving at the last minute of the origin window
        //                          |-----v2----|
        //             >>>>>>>>>>x
        // |-----v1----|
        // |           |            |           |
        // 3           6            10          13

        let origin_start = DateTime::from_str("2021-01-01T03:00:00Z").unwrap();
        let origin_end = DateTime::from_str("2021-01-01T06:00:00Z").unwrap();
        let origin_vertipads = HashMap::from([(
            origin_vertipad_id.clone(),
            vec![Timeslot::new(origin_start, origin_end).unwrap()],
        )]);

        let target_vertipads = HashMap::from([(
            target_vertipad_id.clone(),
            vec![Timeslot::new(
                DateTime::from_str("2021-01-01T10:00:00Z").unwrap(),
                DateTime::from_str("2021-01-01T13:00:00Z").unwrap(),
            )
            .unwrap()],
        )]);

        let pairs = get_vertipad_timeslot_pairs(
            &origin_vertiport_id,
            &target_vertiport_id,
            origin_vertipads,
            target_vertipads,
            &clients,
        )
        .await
        .unwrap();

        assert!(pairs.is_empty());
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn ut_get_vertipad_pairs_no_overlap_target_lead() {
        let origin_vertiport_id: String = Uuid::new_v4().to_string();
        let target_vertiport_id: String = Uuid::new_v4().to_string();
        let origin_vertipad_id: String = Uuid::new_v4().to_string();
        let target_vertipad_id: String = Uuid::new_v4().to_string();
        let clients = get_clients().await;

        //
        // No overlap, target window is earlier
        //             |-----v1----|
        //
        // |-----v2----|
        // |           |           |           |
        // 3           6           10          13

        let origin_start = DateTime::from_str("2021-01-01T06:00:00Z").unwrap();
        let origin_end = DateTime::from_str("2021-01-01T10:00:00Z").unwrap();
        let origin_vertipads = HashMap::from([(
            origin_vertipad_id.clone(),
            vec![Timeslot::new(origin_start, origin_end).unwrap()],
        )]);

        let target_vertipads = HashMap::from([(
            target_vertipad_id.clone(),
            vec![Timeslot::new(
                DateTime::from_str("2021-01-01T03:00:00Z").unwrap(),
                DateTime::from_str("2021-01-01T06:00:00Z").unwrap(),
            )
            .unwrap()],
        )]);

        let pairs = get_vertipad_timeslot_pairs(
            &origin_vertiport_id,
            &target_vertiport_id,
            origin_vertipads,
            target_vertipads,
            &clients,
        )
        .await
        .unwrap();

        println!("{:?}", pairs);
        assert!(pairs.is_empty());
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn ut_get_vertipad_pairs_some_overlap() {
        let origin_vertiport_id: String = Uuid::new_v4().to_string();
        let target_vertiport_id: String = Uuid::new_v4().to_string();
        let origin_vertipad_id: String = Uuid::new_v4().to_string();
        let target_vertipad_id: String = Uuid::new_v4().to_string();
        let clients = get_clients().await;

        //
        // Some overlap
        //             |-----v2----|
        //             >>>>> Leave at end of depart window
        //           >>>>> Middle case
        //         >>>>> Arrive at start of target window
        // |-----v1----|
        // |           |           |
        // 3           6           9

        let origin_start = DateTime::from_str("2021-01-01T03:00:00Z").unwrap();
        let origin_end = DateTime::from_str("2021-01-01T06:00:00Z").unwrap();
        let origin_vertipads = HashMap::from([(
            origin_vertipad_id.clone(),
            vec![Timeslot::new(origin_start, origin_end).unwrap()],
        )]);

        let target_start = DateTime::from_str("2021-01-01T06:00:00Z").unwrap();
        let target_end = DateTime::from_str("2021-01-01T09:00:00Z").unwrap();
        let target_vertipads = HashMap::from([(
            target_vertipad_id.clone(),
            vec![Timeslot::new(target_start, target_end).unwrap()],
        )]);

        let pairs = get_vertipad_timeslot_pairs(
            &origin_vertiport_id,
            &target_vertiport_id,
            origin_vertipads,
            target_vertipads,
            &clients,
        )
        .await
        .unwrap();

        assert_eq!(pairs.len(), 1);
        let pair = pairs.last().unwrap();
        let flight_duration = estimate_flight_time_seconds(&pair.distance_meters).unwrap();

        assert_eq!(pair.origin_vertipad_id, origin_vertipad_id);
        assert_eq!(pair.target_vertipad_id, target_vertipad_id);
        assert_eq!(
            pair.origin_timeslot.time_start(),
            target_start - flight_duration
        );

        assert_eq!(pair.origin_timeslot.time_end(), origin_end);
        assert_eq!(pair.target_timeslot.time_start(), target_start);
        assert_eq!(
            pair.target_timeslot.time_end(),
            origin_end + flight_duration
        );
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn ut_get_vertipad_pairs_overlap_nested() {
        let origin_vertiport_id: String = Uuid::new_v4().to_string();
        let target_vertiport_id: String = Uuid::new_v4().to_string();
        let origin_vertipad_id: String = Uuid::new_v4().to_string();
        let target_vertipad_id: String = Uuid::new_v4().to_string();
        let clients = get_clients().await;
        //
        // Some overlap
        //       |-----v2---|
        //                >>> Arrive at end of target window
        //         >>> Middle case
        //     >>> Arrive at start of target window
        // |-----v1----------------|
        // |           |           |
        // 3           6           9

        let origin_start = DateTime::from_str("2021-01-01T03:00:00Z").unwrap();
        let origin_end = DateTime::from_str("2021-01-01T09:00:00Z").unwrap();
        let origin_vertipads = HashMap::from([(
            origin_vertipad_id.clone(),
            vec![Timeslot::new(origin_start, origin_end).unwrap()],
        )]);

        let target_start = DateTime::from_str("2021-01-01T05:00:00Z").unwrap();
        let target_end = DateTime::from_str("2021-01-01T07:00:00Z").unwrap();
        let target_vertipads = HashMap::from([(
            target_vertipad_id.clone(),
            vec![Timeslot::new(target_start, target_end).unwrap()],
        )]);

        let pairs = get_vertipad_timeslot_pairs(
            &origin_vertiport_id,
            &target_vertiport_id,
            origin_vertipads,
            target_vertipads,
            &clients,
        )
        .await
        .unwrap();

        assert_eq!(pairs.len(), 1);
        let pair = pairs.last().unwrap();
        let flight_duration = estimate_flight_time_seconds(&pair.distance_meters).unwrap();

        assert_eq!(pair.origin_vertipad_id, origin_vertipad_id);
        assert_eq!(pair.target_vertipad_id, target_vertipad_id);
        assert_eq!(
            pair.origin_timeslot.time_start(),
            target_start - flight_duration
        );
        assert_eq!(
            pair.origin_timeslot.time_end(),
            target_end - flight_duration
        );
        assert_eq!(pair.target_timeslot.time_start(), target_start);
        assert_eq!(pair.target_timeslot.time_end(), target_end);
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn ut_get_vertipad_pairs_overlap_target_window_lead() {
        let origin_vertiport_id: String = Uuid::new_v4().to_string();
        let target_vertiport_id: String = Uuid::new_v4().to_string();
        let origin_vertipad_id: String = Uuid::new_v4().to_string();
        let target_vertipad_id: String = Uuid::new_v4().to_string();
        let clients = get_clients().await;

        //
        // Some overlap, target window leads
        // |-------v2-----|
        //             >>>
        //             |-----------|
        // |           |           |
        // 3           6           9

        let origin_start = DateTime::from_str("2021-01-01T06:00:00Z").unwrap();
        let origin_end = DateTime::from_str("2021-01-01T09:00:00Z").unwrap();
        let origin_vertipads = HashMap::from([(
            origin_vertipad_id.clone(),
            vec![Timeslot::new(origin_start, origin_end).unwrap()],
        )]);

        let target_start = DateTime::from_str("2021-01-01T03:00:00Z").unwrap();
        let target_end = DateTime::from_str("2021-01-01T07:00:00Z").unwrap();
        let target_vertipads = HashMap::from([(
            target_vertipad_id.clone(),
            vec![Timeslot::new(target_start, target_end).unwrap()],
        )]);

        let pairs = get_vertipad_timeslot_pairs(
            &origin_vertiport_id,
            &target_vertiport_id,
            origin_vertipads,
            target_vertipads,
            &clients,
        )
        .await
        .unwrap();

        assert_eq!(pairs.len(), 1);
        let pair = pairs.last().unwrap();
        let flight_duration = estimate_flight_time_seconds(&pair.distance_meters).unwrap();

        assert_eq!(pair.origin_vertipad_id, origin_vertipad_id);
        assert_eq!(pair.target_vertipad_id, target_vertipad_id);
        assert_eq!(pair.origin_timeslot.time_start(), origin_start);
        assert_eq!(
            pair.origin_timeslot.time_end(),
            target_end - flight_duration
        );
        assert_eq!(
            pair.target_timeslot.time_start(),
            origin_start + flight_duration
        );
        assert_eq!(pair.target_timeslot.time_end(), target_end);
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn ut_get_vertipad_pairs_overlap_multiple() {
        let origin_vertiport_id: String = Uuid::new_v4().to_string();
        let target_vertiport_id: String = Uuid::new_v4().to_string();
        let origin_vertipad_id: String = Uuid::new_v4().to_string();
        let target_vertipad_id: String = Uuid::new_v4().to_string();
        let clients = get_clients().await;

        //
        // Some overlap
        //       |-----v2-p1--|    |-----v2-p2--|
        //
        // |-----v1----------------|
        // |           |           |
        // 3           6           9

        let origin_start = DateTime::from_str("2021-01-01T03:00:00Z").unwrap();
        let origin_end = DateTime::from_str("2021-01-01T09:00:00Z").unwrap();
        let origin_vertipads = HashMap::from([(
            origin_vertipad_id.clone(),
            vec![Timeslot::new(origin_start, origin_end).unwrap()],
        )]);

        let target_timeslot_1 = Timeslot::new(
            DateTime::from_str("2021-01-01T05:00:00Z").unwrap(),
            DateTime::from_str("2021-01-01T07:00:00Z").unwrap(),
        )
        .unwrap();

        let target_timeslot_2 = Timeslot::new(
            DateTime::from_str("2021-01-01T09:00:00Z").unwrap(),
            DateTime::from_str("2021-01-01T10:00:00Z").unwrap(),
        )
        .unwrap();

        let target_vertipads = HashMap::from([(
            target_vertipad_id.clone(),
            vec![target_timeslot_1, target_timeslot_2],
        )]);

        let pairs = get_vertipad_timeslot_pairs(
            &origin_vertiport_id,
            &target_vertiport_id,
            origin_vertipads,
            target_vertipads,
            &clients,
        )
        .await
        .unwrap();

        assert_eq!(pairs.len(), 2);

        {
            let pair = pairs[0].clone();
            let target_timeslot = target_timeslot_1;
            assert_eq!(pair.origin_vertipad_id, origin_vertipad_id);
            assert_eq!(pair.target_vertipad_id, target_vertipad_id);

            let flight_duration = estimate_flight_time_seconds(&pair.distance_meters).unwrap();
            assert_eq!(
                pair.origin_timeslot.time_start(),
                target_timeslot.time_start() - flight_duration
            );

            assert_eq!(
                pair.origin_timeslot.time_end(),
                target_timeslot.time_end() - flight_duration
            );

            assert_eq!(
                pair.target_timeslot.time_start(),
                target_timeslot.time_start()
            );
            assert_eq!(pair.target_timeslot.time_end(), target_timeslot.time_end());
        }

        {
            let pair = pairs[1].clone();
            let target_timeslot = target_timeslot_2;
            assert_eq!(pair.origin_vertipad_id, origin_vertipad_id);
            assert_eq!(pair.target_vertipad_id, target_vertipad_id);

            let flight_duration = estimate_flight_time_seconds(&pair.distance_meters).unwrap();
            assert_eq!(
                pair.origin_timeslot.time_start(),
                target_timeslot.time_start() - flight_duration
            );

            assert_eq!(pair.origin_timeslot.time_end(), origin_end);

            assert_eq!(
                pair.target_timeslot.time_start(),
                target_timeslot.time_start()
            );
            assert_eq!(
                pair.target_timeslot.time_end(),
                origin_end + flight_duration
            );
        }
    }

    #[test]
    fn test_vertiport_error_display() {
        assert_eq!(format!("{}", VertiportError::ClientError), "Client error");
        assert_eq!(format!("{}", VertiportError::Data), "Invalid data");
        assert_eq!(format!("{}", VertiportError::NoVertipads), "No vertipads");
        assert_eq!(format!("{}", VertiportError::NoSchedule), "No schedule");
        assert_eq!(format!("{}", VertiportError::Schedule), "Invalid schedule");
        assert_eq!(format!("{}", VertiportError::Internal), "Internal error");
    }

    #[test]
    fn test_from_timeslot_pair_data() {
        let now = Utc::now();
        let val = TimeslotPair {
            waypoints: vec![
                PointZ {
                    latitude: 1.0,
                    longitude: 2.0,
                    altitude_meters: 3.0,
                },
                PointZ {
                    latitude: 4.0,
                    longitude: 5.0,
                    altitude_meters: 6.0,
                },
            ],
            origin_vertiport_id: "origin_vertiport_id".to_string(),
            origin_vertipad_id: "origin_vertipad_id".to_string(),
            origin_timeslot: Timeslot::new(now, now + Duration::try_seconds(1).unwrap()).unwrap(),
            target_vertiport_id: "target_vertiport_id".to_string(),
            target_vertipad_id: "target_vertipad_id".to_string(),
            target_timeslot: Timeslot::new(
                now + Duration::try_seconds(2).unwrap(),
                now + Duration::try_seconds(3).unwrap(),
            )
            .unwrap(),
            distance_meters: 100.0,
        };

        let data: flight_plan::Data = val.into();
        assert_eq!(data.origin_vertiport_id, "origin_vertiport_id".to_string());
        assert_eq!(data.origin_vertipad_id, "origin_vertipad_id");
        assert_eq!(data.origin_timeslot_start, Some(now.into()));
        assert_eq!(
            data.origin_timeslot_end,
            Some((now + Duration::try_seconds(1).unwrap()).into())
        );
        assert_eq!(data.target_vertiport_id, "target_vertiport_id".to_string());
        assert_eq!(data.target_vertipad_id, "target_vertipad_id");
        assert_eq!(
            data.target_timeslot_start,
            Some((now + Duration::try_seconds(2).unwrap()).into())
        );
        assert_eq!(
            data.target_timeslot_end,
            Some((now + Duration::try_seconds(3).unwrap()).into())
        );

        let waypoints = data.waypoints.unwrap();
        assert_eq!(waypoints.points.len(), 2);
        assert_eq!(waypoints.points[0].y, 1.0);
        assert_eq!(waypoints.points[0].x, 2.0);
        assert_eq!(waypoints.points[0].z, 3.0);
        assert_eq!(waypoints.points[1].y, 4.0);
        assert_eq!(waypoints.points[1].x, 5.0);
        assert_eq!(waypoints.points[1].z, 6.0);
    }
}
