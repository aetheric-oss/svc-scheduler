//! Vertiport-related utilities

use super::flight_plan::*;
use super::schedule::*;
use super::vehicle::*;
use super::{best_path, BestPathError, BestPathRequest};
use crate::grpc::client::GrpcClients;
use chrono::Duration;
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
    ClientError,
    InvalidData,
    NoVertipads,
    NoSchedule,
    InvalidSchedule,
}

impl std::fmt::Display for VertiportError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            VertiportError::ClientError => write!(f, "Client error"),
            VertiportError::InvalidData => write!(f, "Invalid data"),
            VertiportError::NoVertipads => write!(f, "No vertipads"),
            VertiportError::NoSchedule => write!(f, "No schedule"),
            VertiportError::InvalidSchedule => write!(f, "Invalid schedule"),
        }
    }
}

/// Gets all vertipads for a vertiport
pub async fn get_vertipads(
    vertiport_id: &String,
    clients: &GrpcClients,
) -> Result<Vec<String>, VertiportError> {
    let filter =
        AdvancedSearchFilter::search_equals("vertiport_id".to_string(), vertiport_id.to_string())
            .and_is_null("deleted_at".to_owned())
            .and_equals("enabled".to_string(), "1".to_string());
    router_info!("(get_vertipads) proposed filter: {:?}", filter.clone());

    let filter = AdvancedSearchFilter::default();
    let Ok(response) = clients
        .storage
        .vertipad
        .search(filter)
        .await
    else {
            let error_str = format!(
                "Failed to get vertipads for vertiport {}.",
                vertiport_id
            );
            router_error!("(get_vertipads) {}", error_str);
            return Err(VertiportError::NoVertipads);
    };

    router_info!("(get_vertipads) vertiport: {:?}", vertiport_id);
    router_info!("(get_vertipads) response: {:?}", response);

    Ok(response
        .into_inner()
        .list
        .into_iter()
        // in R3 the search filter is not working, do an extra filter here
        .filter_map(|vp| {
            let Some(data) = vp.data else {
                return None;
            };

            if data.vertiport_id != *vertiport_id {
                return None;
            }

            if !data.enabled {
                return None;
            }

            Some(vp.id)
        })
        .collect::<Vec<String>>())
}

/// Get pairs of timeslots where a flight can leave within the departure timeslot
///  and land within the arrival timeslot
pub async fn get_timeslot_pairs(
    departure_vertiport_id: &String,
    arrival_vertiport_id: &String,
    departure_time_block: &Duration,
    arrival_time_block: &Duration,
    timeslot: &Timeslot,
    existing_flight_plans: &[FlightPlanSchedule],
    clients: &GrpcClients,
) -> Result<Vec<TimeslotPair>, VertiportError> {
    let departure_timeslots = get_available_timeslots(
        departure_vertiport_id,
        existing_flight_plans,
        timeslot,
        departure_time_block,
        clients,
    )
    .await?;

    let arrival_timeslots = get_available_timeslots(
        arrival_vertiport_id,
        existing_flight_plans,
        timeslot,
        arrival_time_block,
        clients,
    )
    .await?;

    get_vertipad_timeslot_pairs(
        departure_vertiport_id,
        arrival_vertiport_id,
        departure_timeslots,
        arrival_timeslots,
        clients,
    )
    .await
}

/// Return a map of vertipad ids to available timeslots for that vertipad
///
/// TODO(R4): This will be replaced with a call to svc-storage vertipad_timeslots to
///  return a list of available timeslots for each vertipad, so we don't
///  need to rebuild each pad's schedule from flight plans each time
async fn get_available_timeslots(
    vertiport_id: &String,
    existing_flight_plans: &[FlightPlanSchedule],
    timeslot: &Timeslot,
    minimum_duration: &Duration,
    clients: &GrpcClients,
) -> Result<HashMap<String, Vec<Timeslot>>, VertiportError> {
    // Get vertiport schedule
    let calendar = get_vertiport_calendar(vertiport_id, clients).await?;

    // TODO(R4): Use each vertipad's calendar
    let base_timeslots = calendar.to_timeslots(&timeslot.time_start, &timeslot.time_end);
    router_debug!(
        "(get_available_timeslots) base_timeslots: {:?}",
        base_timeslots
    );

    // TODO(R4): This is currently hardcoded, get the duration of the timeslot
    // try min and max both the necessary landing time
    let max_duration = Duration::minutes(MAX_DURATION_TIMESLOT_MINUTES);

    // Prepare a list of slots for each vertipad
    // For now, each vertipad shares the same schedule as the vertiport itself
    let mut timeslots = get_vertipads(vertiport_id, clients)
        .await?
        .into_iter()
        .map(|id| (id, base_timeslots.clone()))
        .collect::<HashMap<String, Vec<Timeslot>>>();

    // Get occupied slots
    // TODO(R4): This will be replaced with a call to svc-storage vertipad_timeslots to
    //  return a list of occupied timeslots for each vertipad, so we don't
    //  need to rebuild each pad's schedule from flight plans each time
    let occupied_slots = build_timeslots_from_flight_plans(vertiport_id, existing_flight_plans);

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
            router_error!("(get_available_timeslots) Vertipad {} (from a flight plan) not found in list of vertipads from storage.", vertipad_id);
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
    vertiport_id: &String,
    clients: &GrpcClients,
) -> Result<Calendar, VertiportError> {
    let vertiport_object = match clients
        .storage
        .vertiport
        .get_by_id(Id {
            id: vertiport_id.clone(),
        })
        .await
    {
        Ok(response) => response.into_inner(),
        Err(e) => {
            let error_str = format!("Could not retrieve data for vertiport {vertiport_id}.");

            router_error!("(get_vertiport_calendar) {}: {e}", error_str);
            return Err(VertiportError::ClientError);
        }
    };

    let vertiport_data = match vertiport_object.data {
        Some(d) => d,
        None => {
            let error_str = format!("Date invalid for vertiport {}.", vertiport_id);
            router_error!("(get_vertiport_calendar) {}", error_str);
            return Err(VertiportError::InvalidData);
        }
    };

    let Some(vertiport_schedule) = vertiport_data.schedule else {
        let error_str = format!(
            "No schedule for vertiport {}.",
            vertiport_id
        );
        router_error!("(get_vertiport_calendar) {}", error_str);
        return Err(VertiportError::NoSchedule);
    };

    match Calendar::from_str(&vertiport_schedule) {
        Ok(calendar) => Ok(calendar),
        Err(e) => {
            let error_str = format!("Schedule invalid for vertiport {}.", vertiport_id);
            router_error!("(get_vertiport_calendar) {}: {}", error_str, e);
            Err(VertiportError::InvalidSchedule)
        }
    }
}

/// Gets all occupied vertipad time slots given flight plans.
///  If `invert` is true, returns all unoccupied time slots.
///
/// TODO(R4): Remove in favor of read from storage vertipad_timeslot table
///  where the duration of the timeslot is stored
fn build_timeslots_from_flight_plans(
    vertiport_id: &String,
    flight_plans: &[FlightPlanSchedule],
) -> Vec<(String, Timeslot)> {
    // TODO(R4): This is currently hardcoded, get the duration of the timeslot
    //  directly from the vertipad_timeslot object
    let required_loading_time =
        Duration::seconds(crate::grpc::api::query_flight::LOADING_AND_TAKEOFF_TIME_SECONDS);
    let required_unloading_time =
        Duration::seconds(crate::grpc::api::query_flight::LANDING_AND_UNLOADING_TIME_SECONDS);

    flight_plans
        .iter()
        .filter_map(|fp| {
            if *vertiport_id == fp.departure_vertiport_id {
                let timeslot = Timeslot {
                    time_start: fp.departure_time,
                    // TODO(R4): duration should be retrieved from flight plan object
                    //  instead of being hardcoded
                    time_end: fp.departure_time + required_loading_time,
                };

                Some((fp.departure_vertipad_id.clone(), timeslot))
            } else if *vertiport_id == fp.arrival_vertiport_id {
                let timeslot = Timeslot {
                    time_start: fp.arrival_time,
                    // TODO(R4): duration should be retrieved from flight plan object
                    //  instead of being hardcoded
                    time_end: fp.arrival_time + required_unloading_time,
                };

                Some((fp.arrival_vertipad_id.clone(), timeslot))
            } else {
                None
            }
        })
        .collect::<Vec<(String, Timeslot)>>()
}

/// Gets all available timeslot pairs and a path for each pair
#[derive(Debug, Clone)]
pub struct TimeslotPair {
    pub depart_port_id: String,
    pub depart_pad_id: String,
    pub depart_timeslot: Timeslot,
    pub arrival_port_id: String,
    pub arrival_pad_id: String,
    pub arrival_timeslot: Timeslot,
    pub path: GeoLineString,
    pub distance_meters: f32,
}

/// Attempts to find a pairing of departure and arrival pad
///  timeslots wherein a flight could occur.
pub async fn get_vertipad_timeslot_pairs(
    depart_vertiport_id: &String,
    arrival_vertiport_id: &String,
    mut depart_vertipads: HashMap<String, Vec<Timeslot>>,
    mut arrive_vertipads: HashMap<String, Vec<Timeslot>>,
    clients: &GrpcClients,
) -> Result<Vec<TimeslotPair>, VertiportError> {
    let mut pairs = vec![];

    let mut best_path_request = BestPathRequest {
        node_start_id: depart_vertiport_id.clone(),
        node_uuid_end: arrival_vertiport_id.clone(),
        start_type: NodeType::Vertiport as i32,
        time_start: None,
        time_end: None,
    };

    // Iterate through departure pads and their schedules
    for (depart_pad_id, depart_schedule) in depart_vertipads.iter_mut() {
        depart_schedule.sort_by(|a, b| a.time_end.cmp(&b.time_end));

        // Iterate through the available timeslots for this pad
        'depart_timeslots: for dts in depart_schedule.iter() {
            // Iterate through arrival pads and their schedules
            for (arrival_pad_id, arrival_schedule) in arrive_vertipads.iter_mut() {
                arrival_schedule.sort_by(|a, b| a.time_start.cmp(&b.time_start));

                // Iterate through available timeslots for this pad
                // There will be several opportunities to break out without
                //  excess work
                for ats in arrival_schedule.iter() {
                    // no timeslot overlap possible
                    //                    | departure timeslot |
                    // | arrival timeslot |
                    if dts.time_start >= ats.time_end {
                        continue;
                    }

                    // Temporary no-fly zones make checking the same route
                    //  multiple times necessary for different timeslots
                    best_path_request.time_start = Some(dts.time_start.into());
                    best_path_request.time_end = Some(ats.time_end.into());

                    let (path, distance_meters) = match best_path(&best_path_request, clients).await
                    {
                        Ok((path, distance_meters)) => (path, distance_meters as f32),
                        Err(BestPathError::NoPathFound) => {
                            // no path found, perhaps temporary no-fly zone
                            //  is blocking journeys from this depart timeslot
                            // Break out and try the next depart timeslot
                            router_debug!(
                                "(get_vertipad_timeslot_pairs) No path found from vertiport {}
                                to vertiport {} (from {} to {}).",
                                depart_vertiport_id,
                                arrival_vertiport_id,
                                dts.time_start,
                                ats.time_end
                            );

                            break 'depart_timeslots;
                        }
                        Err(BestPathError::ClientError) => {
                            // exit immediately if svc-gis is down, don't allow new flights
                            router_error!(
                                "(get_vertipad_timeslot_pairs) Could not determine path."
                            );
                            return Err(VertiportError::ClientError);
                        }
                    };

                    let estimated_duration_s = estimate_flight_time_seconds(&distance_meters);

                    // Since both schedules are sorted, we can break early once
                    //  departure end time + flight time is less than the arrival timeslot's start time
                    //  and not look at the other timeslots for that pad
                    // | departure timeslot |
                    //                      ---->x
                    //                                | arrival timeslot 1 | arrival timeslot 2 |
                    // (the next arrival timeslot start to be checked would be even further away)
                    if dts.time_end + estimated_duration_s < ats.time_start {
                        break;
                    }

                    //
                    // |     dts              |          (depart timeslot)
                    //       ----->        ----->        (flight time)
                    //            |      ats     |       (arrival timeslot)
                    //       | actual dts  |             (actual depart timeslot)
                    //
                    // The actual depart_timeslot is the timeslot within which departure
                    //  will result in landing in the arrival timeslot.
                    let depart_timeslot = Timeslot {
                        time_start: max(dts.time_start, ats.time_start - estimated_duration_s),
                        time_end: min(dts.time_end, ats.time_end - estimated_duration_s),
                    };

                    //
                    //  |     dts     |             (depart timeslot)
                    //   ----->       ----->        (flight time)
                    //      |    ats            |   (arrival timeslot)
                    //         | actual ats |
                    // The actual arrival_timeslot is the timeslot within which arrival is possible
                    //  given a departure from the actual depart timeslot.
                    let arrival_timeslot = Timeslot {
                        time_start: max(
                            ats.time_start,
                            depart_timeslot.time_start + estimated_duration_s,
                        ),
                        time_end: min(
                            ats.time_end,
                            depart_timeslot.time_end + estimated_duration_s,
                        ),
                    };

                    pairs.push(TimeslotPair {
                        depart_port_id: depart_vertiport_id.clone(),
                        depart_pad_id: depart_pad_id.clone(),
                        depart_timeslot,
                        arrival_port_id: arrival_vertiport_id.clone(),
                        arrival_pad_id: arrival_pad_id.clone(),
                        arrival_timeslot,
                        path,
                        distance_meters,
                    });
                }
            }
        }
    }

    // Sort available options by shortest distance first
    pairs.sort_by(
        |a, b| match a.distance_meters.partial_cmp(&b.distance_meters) {
            Some(ord) => ord,
            None => {
                router_error!(
                    "(get_vertipad_timeslot_pairs) Could not compare distances: {}, {}",
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
    use chrono::DateTime;
    use uuid::Uuid;

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn ut_get_vertipad_pairs_no_overlap() {
        let depart_vertiport_id: String = Uuid::new_v4().to_string();
        let arrive_vertiport_id: String = Uuid::new_v4().to_string();
        let depart_vertipad_id: String = Uuid::new_v4().to_string();
        let arrive_vertipad_id: String = Uuid::new_v4().to_string();
        let clients = get_clients().await;

        //
        // CASE 1: No overlap, even leaving at the last minute of the departure window
        //                          |-----v2----|
        //             >>>>>>>>>>x
        // |-----v1----|
        // |           |            |           |
        // 3           6            10          13

        let depart_start = DateTime::from_str("2021-01-01T03:00:00Z").unwrap();
        let depart_end = DateTime::from_str("2021-01-01T06:00:00Z").unwrap();
        let depart_vertipads = HashMap::from([(
            depart_vertipad_id.clone(),
            vec![Timeslot {
                time_start: depart_start,
                time_end: depart_end,
            }],
        )]);

        let arrival_vertipads = HashMap::from([(
            arrive_vertipad_id.clone(),
            vec![Timeslot {
                time_start: DateTime::from_str("2021-01-01T10:00:00Z").unwrap(),
                time_end: DateTime::from_str("2021-01-01T13:00:00Z").unwrap(),
            }],
        )]);

        let pairs = get_vertipad_timeslot_pairs(
            &depart_vertiport_id,
            &arrive_vertiport_id,
            depart_vertipads,
            arrival_vertipads,
            &clients,
        )
        .await
        .unwrap();

        assert!(pairs.is_empty());
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn ut_get_vertipad_pairs_no_overlap_arrival_lead() {
        let depart_vertiport_id: String = Uuid::new_v4().to_string();
        let arrive_vertiport_id: String = Uuid::new_v4().to_string();
        let depart_vertipad_id: String = Uuid::new_v4().to_string();
        let arrive_vertipad_id: String = Uuid::new_v4().to_string();
        let clients = get_clients().await;

        //
        // No overlap, arrival window is earlier
        //             |-----v1----|
        //
        // |-----v2----|
        // |           |           |           |
        // 3           6           10          13

        let depart_start = DateTime::from_str("2021-01-01T06:00:00Z").unwrap();
        let depart_end = DateTime::from_str("2021-01-01T10:00:00Z").unwrap();
        let depart_vertipads = HashMap::from([(
            depart_vertipad_id.clone(),
            vec![Timeslot {
                time_start: depart_start,
                time_end: depart_end,
            }],
        )]);

        let arrival_vertipads = HashMap::from([(
            arrive_vertipad_id.clone(),
            vec![Timeslot {
                time_start: DateTime::from_str("2021-01-01T03:00:00Z").unwrap(),
                time_end: DateTime::from_str("2021-01-01T06:00:00Z").unwrap(),
            }],
        )]);

        let pairs = get_vertipad_timeslot_pairs(
            &depart_vertiport_id,
            &arrive_vertiport_id,
            depart_vertipads,
            arrival_vertipads,
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
        let depart_vertiport_id: String = Uuid::new_v4().to_string();
        let arrive_vertiport_id: String = Uuid::new_v4().to_string();
        let depart_vertipad_id: String = Uuid::new_v4().to_string();
        let arrive_vertipad_id: String = Uuid::new_v4().to_string();
        let clients = get_clients().await;

        //
        // Some overlap
        //             |-----v2----|
        //             >>>>> Leave at end of depart window
        //           >>>>> Middle case
        //         >>>>> Arrive at start of arrival window
        // |-----v1----|
        // |           |           |
        // 3           6           9

        let depart_start = DateTime::from_str("2021-01-01T03:00:00Z").unwrap();
        let depart_end = DateTime::from_str("2021-01-01T06:00:00Z").unwrap();
        let depart_vertipads = HashMap::from([(
            depart_vertipad_id.clone(),
            vec![Timeslot {
                time_start: depart_start,
                time_end: depart_end,
            }],
        )]);

        let arrival_start = DateTime::from_str("2021-01-01T06:00:00Z").unwrap();
        let arrival_end = DateTime::from_str("2021-01-01T09:00:00Z").unwrap();
        let arrival_vertipads = HashMap::from([(
            arrive_vertipad_id.clone(),
            vec![Timeslot {
                time_start: arrival_start,
                time_end: arrival_end,
            }],
        )]);

        let pairs = get_vertipad_timeslot_pairs(
            &depart_vertiport_id,
            &arrive_vertiport_id,
            depart_vertipads,
            arrival_vertipads,
            &clients,
        )
        .await
        .unwrap();

        assert_eq!(pairs.len(), 1);
        let pair = pairs.last().unwrap();
        let flight_duration = estimate_flight_time_seconds(&pair.distance_meters);

        assert_eq!(pair.depart_pad_id, depart_vertipad_id);
        assert_eq!(pair.arrival_pad_id, arrive_vertipad_id);
        assert_eq!(
            pair.depart_timeslot.time_start,
            arrival_start - flight_duration
        );

        assert_eq!(pair.depart_timeslot.time_end, depart_end);
        assert_eq!(pair.arrival_timeslot.time_start, arrival_start);
        assert_eq!(pair.arrival_timeslot.time_end, depart_end + flight_duration);
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn ut_get_vertipad_pairs_overlap_nested() {
        let depart_vertiport_id: String = Uuid::new_v4().to_string();
        let arrive_vertiport_id: String = Uuid::new_v4().to_string();
        let depart_vertipad_id: String = Uuid::new_v4().to_string();
        let arrive_vertipad_id: String = Uuid::new_v4().to_string();
        let clients = get_clients().await;
        //
        // Some overlap
        //       |-----v2---|
        //                >>> Arrive at end of arrival window
        //         >>> Middle case
        //     >>> Arrive at start of arrival window
        // |-----v1----------------|
        // |           |           |
        // 3           6           9

        let depart_start = DateTime::from_str("2021-01-01T03:00:00Z").unwrap();
        let depart_end = DateTime::from_str("2021-01-01T09:00:00Z").unwrap();
        let depart_vertipads = HashMap::from([(
            depart_vertipad_id.clone(),
            vec![Timeslot {
                time_start: depart_start,
                time_end: depart_end,
            }],
        )]);

        let arrival_start = DateTime::from_str("2021-01-01T05:00:00Z").unwrap();
        let arrival_end = DateTime::from_str("2021-01-01T07:00:00Z").unwrap();
        let arrival_vertipads = HashMap::from([(
            arrive_vertipad_id.clone(),
            vec![Timeslot {
                time_start: arrival_start,
                time_end: arrival_end,
            }],
        )]);

        let pairs = get_vertipad_timeslot_pairs(
            &depart_vertiport_id,
            &arrive_vertiport_id,
            depart_vertipads,
            arrival_vertipads,
            &clients,
        )
        .await
        .unwrap();

        assert_eq!(pairs.len(), 1);
        let pair = pairs.last().unwrap();
        let flight_duration = estimate_flight_time_seconds(&pair.distance_meters);

        assert_eq!(pair.depart_pad_id, depart_vertipad_id);
        assert_eq!(pair.arrival_pad_id, arrive_vertipad_id);
        assert_eq!(
            pair.depart_timeslot.time_start,
            arrival_start - flight_duration
        );
        assert_eq!(pair.depart_timeslot.time_end, arrival_end - flight_duration);
        assert_eq!(pair.arrival_timeslot.time_start, arrival_start);
        assert_eq!(pair.arrival_timeslot.time_end, arrival_end);
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn ut_get_vertipad_pairs_overlap_arrival_window_lead() {
        let depart_vertiport_id: String = Uuid::new_v4().to_string();
        let arrive_vertiport_id: String = Uuid::new_v4().to_string();
        let depart_vertipad_id: String = Uuid::new_v4().to_string();
        let arrive_vertipad_id: String = Uuid::new_v4().to_string();
        let clients = get_clients().await;

        //
        // Some overlap, arrival window leads
        // |-------v2-----|
        //             >>>
        //             |-----------|
        // |           |           |
        // 3           6           9

        let depart_start = DateTime::from_str("2021-01-01T06:00:00Z").unwrap();
        let depart_end = DateTime::from_str("2021-01-01T09:00:00Z").unwrap();
        let depart_vertipads = HashMap::from([(
            depart_vertipad_id.clone(),
            vec![Timeslot {
                time_start: depart_start,
                time_end: depart_end,
            }],
        )]);

        let arrival_start = DateTime::from_str("2021-01-01T03:00:00Z").unwrap();
        let arrival_end = DateTime::from_str("2021-01-01T07:00:00Z").unwrap();
        let arrival_vertipads = HashMap::from([(
            arrive_vertipad_id.clone(),
            vec![Timeslot {
                time_start: arrival_start,
                time_end: arrival_end,
            }],
        )]);

        let pairs = get_vertipad_timeslot_pairs(
            &depart_vertiport_id,
            &arrive_vertiport_id,
            depart_vertipads,
            arrival_vertipads,
            &clients,
        )
        .await
        .unwrap();

        assert_eq!(pairs.len(), 1);
        let pair = pairs.last().unwrap();
        let flight_duration = estimate_flight_time_seconds(&pair.distance_meters);

        assert_eq!(pair.depart_pad_id, depart_vertipad_id);
        assert_eq!(pair.arrival_pad_id, arrive_vertipad_id);
        assert_eq!(pair.depart_timeslot.time_start, depart_start);
        assert_eq!(pair.depart_timeslot.time_end, arrival_end - flight_duration);
        assert_eq!(
            pair.arrival_timeslot.time_start,
            depart_start + flight_duration
        );
        assert_eq!(pair.arrival_timeslot.time_end, arrival_end);
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn ut_get_vertipad_pairs_overlap_multiple() {
        let depart_vertiport_id: String = Uuid::new_v4().to_string();
        let arrive_vertiport_id: String = Uuid::new_v4().to_string();
        let depart_vertipad_id: String = Uuid::new_v4().to_string();
        let arrive_vertipad_id: String = Uuid::new_v4().to_string();
        let clients = get_clients().await;

        //
        // Some overlap
        //       |-----v2-p1--|    |-----v2-p2--|
        //
        // |-----v1----------------|
        // |           |           |
        // 3           6           9

        let depart_start = DateTime::from_str("2021-01-01T03:00:00Z").unwrap();
        let depart_end = DateTime::from_str("2021-01-01T09:00:00Z").unwrap();
        let depart_vertipads = HashMap::from([(
            depart_vertipad_id.clone(),
            vec![Timeslot {
                time_start: depart_start,
                time_end: depart_end,
            }],
        )]);

        let arrival_timeslot_1 = Timeslot {
            time_start: DateTime::from_str("2021-01-01T05:00:00Z").unwrap(),
            time_end: DateTime::from_str("2021-01-01T07:00:00Z").unwrap(),
        };

        let arrival_timeslot_2 = Timeslot {
            time_start: DateTime::from_str("2021-01-01T09:00:00Z").unwrap(),
            time_end: DateTime::from_str("2021-01-01T10:00:00Z").unwrap(),
        };

        let arrival_vertipads = HashMap::from([(
            arrive_vertipad_id.clone(),
            vec![arrival_timeslot_1, arrival_timeslot_2],
        )]);

        let pairs = get_vertipad_timeslot_pairs(
            &depart_vertiport_id,
            &arrive_vertiport_id,
            depart_vertipads,
            arrival_vertipads,
            &clients,
        )
        .await
        .unwrap();

        assert_eq!(pairs.len(), 2);

        {
            let pair = pairs[0].clone();
            let arrival_timeslot = arrival_timeslot_1;
            assert_eq!(pair.depart_pad_id, depart_vertipad_id);
            assert_eq!(pair.arrival_pad_id, arrive_vertipad_id);

            let flight_duration = estimate_flight_time_seconds(&pair.distance_meters);
            assert_eq!(
                pair.depart_timeslot.time_start,
                arrival_timeslot.time_start - flight_duration
            );

            assert_eq!(
                pair.depart_timeslot.time_end,
                arrival_timeslot.time_end - flight_duration
            );

            assert_eq!(
                pair.arrival_timeslot.time_start,
                arrival_timeslot.time_start
            );
            assert_eq!(pair.arrival_timeslot.time_end, arrival_timeslot.time_end);
        }

        {
            let pair = pairs[1].clone();
            let arrival_timeslot = arrival_timeslot_2;
            assert_eq!(pair.depart_pad_id, depart_vertipad_id);
            assert_eq!(pair.arrival_pad_id, arrive_vertipad_id);

            let flight_duration = estimate_flight_time_seconds(&pair.distance_meters);
            assert_eq!(
                pair.depart_timeslot.time_start,
                arrival_timeslot.time_start - flight_duration
            );

            assert_eq!(pair.depart_timeslot.time_end, depart_end);

            assert_eq!(
                pair.arrival_timeslot.time_start,
                arrival_timeslot.time_start
            );
            assert_eq!(pair.arrival_timeslot.time_end, depart_end + flight_duration);
        }
    }
}
