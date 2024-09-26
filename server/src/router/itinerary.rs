//! Build an itinerary given aircraft availability and the flight window

use super::flight_plan::FlightPlanSchedule;
use super::schedule::*;
use super::vehicle::*;
use super::vertiport::TimeslotPair;
use super::{best_path, BestPathError, BestPathRequest};
use crate::grpc::client::GrpcClients;
use svc_gis_client_grpc::prelude::gis::*;
use svc_storage_client_grpc::prelude::*;

use lib_common::time::{DateTime, Duration, Utc};
use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter, Result as FmtResult};

const MAX_ITINERARIES: usize = 2;

/// Errors that may occur while processing an itinerary
#[derive(Debug, Clone, PartialEq)]
pub enum ItineraryError {
    /// There was an error contacting a dependency
    ClientError,

    /// The provided data was invalid
    Data,

    /// The vehicle id was inconsistent
    VehicleId,

    /// Inconsistent vertipads
    Vertipads,

    /// Invalid time windows
    TimeWindow,

    /// No path provided
    NoPath,

    /// Path is too short
    PathTooShort,

    /// No path could be found between the origin and target vertipads
    NoPathFound,

    /// There was a schedule conflict
    ScheduleConflict,

    /// An internal error occurred
    Internal,
}

impl Display for ItineraryError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            ItineraryError::ClientError => write!(f, "Could not contact dependency."),
            ItineraryError::Data => write!(f, "Invalid data."),
            ItineraryError::VehicleId => write!(f, "Inconsistent vehicle ID."),
            ItineraryError::Vertipads => write!(f, "Inconsistent vertipads."),
            ItineraryError::TimeWindow => write!(f, "Invalid time window."),
            ItineraryError::NoPath => write!(f, "No path provided."),
            ItineraryError::PathTooShort => write!(f, "Path is too short."),
            ItineraryError::NoPathFound => write!(f, "No path found."),
            ItineraryError::ScheduleConflict => write!(f, "Schedule conflict."),
            ItineraryError::Internal => write!(f, "Internal error."),
        }
    }
}

// Verify that the provided flight plans are structured correctly
// 1) A single aircraft per itinerary.
// 2) A connecting flight plan should leave from the same pad the previous flight plan landed on.
// 3) Should be in order of departure time.
pub fn validate_itinerary(
    flight_plans: &[FlightPlanSchedule],
    vertipad_ids: &mut HashSet<String>,
    aircraft_id: &mut String,
) -> Result<(), ItineraryError> {
    if flight_plans.is_empty() {
        router_error!("No flight plans provided.");
        return Err(ItineraryError::Data);
    }

    if flight_plans.len() == 1 {
        router_debug!("Only one flight plan provided.");
        *aircraft_id = flight_plans[0].vehicle_id.clone();
        vertipad_ids.insert(flight_plans[0].origin_vertipad_id.clone());
        vertipad_ids.insert(flight_plans[0].target_vertipad_id.clone());
        return Ok(());
    }

    for fps in flight_plans.windows(2) {
        let fp_1 = &fps[0];
        let fp_2 = &fps[1];
        if fp_1.target_vertipad_id != fp_2.origin_vertipad_id {
            let error_msg = "Flight plan arrivals and departures don't match";
            router_error!(
                "{error_msg}: {} -> {}",
                fp_1.target_vertipad_id,
                fp_2.origin_vertipad_id
            );

            return Err(ItineraryError::Vertipads);
        }

        vertipad_ids.insert(fp_1.origin_vertipad_id.clone());
        vertipad_ids.insert(fp_1.target_vertipad_id.clone());
        vertipad_ids.insert(fp_2.origin_vertipad_id.clone());
        vertipad_ids.insert(fp_2.target_vertipad_id.clone());

        if aircraft_id.is_empty() {
            *aircraft_id = fp_1.vehicle_id.clone();
        }

        if *aircraft_id != fp_2.vehicle_id {
            router_error!(
                "Flight plans should use the same aircraft: {:#?}",
                flight_plans
            );

            return Err(ItineraryError::VehicleId);
        }

        if fp_1.origin_timeslot_start >= fp_1.target_timeslot_start {
            router_error!(
                "Flight plans should be in order of departure time: {:#?}",
                flight_plans
            );

            return Err(ItineraryError::TimeWindow);
        }

        if fp_1.target_timeslot_end > fp_2.origin_timeslot_start {
            router_error!(
                "Flight plans should be in order of departure time: {:#?}",
                flight_plans
            );

            return Err(ItineraryError::TimeWindow);
        }

        for fp in [fp_1, fp_2] {
            let length = fp
                .waypoints
                .as_ref()
                .ok_or_else(|| {
                    router_error!("Flight plan should have waypoints: {:#?}", flight_plans);
                    ItineraryError::NoPath
                })?
                .len();

            // The waypoints should start and end with the mandatory egress and ingress waypoints of
            //  the vertiport/pad.
            if length < 2 {
                router_error!(
                    "Flight plan path needs two or more points: {:#?}",
                    flight_plans
                );

                return Err(ItineraryError::PathTooShort);
            }
        }
    }

    if aircraft_id.is_empty() {
        router_error!("No aircraft id found.");
        return Err(ItineraryError::Data);
    }

    Ok(())
}

/// Given timeslot pairs for departure and arrival vertiport and the
///  availabilities of the aircraft, get possible itineraries for each
///  aircraft.
/// Returns a maximum of 1 itinerary per aircraft.
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need running backend, integration tests
pub async fn calculate_itineraries(
    required_loading_time: &Duration,
    required_unloading_time: &Duration,
    timeslot_pairs: &[TimeslotPair],
    aircraft_gaps: &HashMap<String, Vec<Availability>>,
    clients: &GrpcClients,
) -> Result<Vec<Vec<flight_plan::Data>>, ItineraryError> {
    let mut itineraries: Vec<Vec<flight_plan::Data>> = vec![];
    let mut ordered: Vec<(String, Availability)> = aircraft_gaps
        .iter()
        .flat_map(|(k, vs)| {
            vs.iter()
                .map(|v| (k.clone(), v.to_owned()))
                .collect::<Vec<(String, Availability)>>()
        })
        .collect();

    ordered.sort_by(|a, b| a.1.timeslot.time_start().cmp(&b.1.timeslot.time_start()));

    // For each available aircraft, see if it can do the flight
    'outer: for pair in timeslot_pairs {
        // TODO(R5): Include vehicle model to improve estimate
        let flight_duration = estimate_flight_time_seconds(&pair.distance_meters).map_err(|e| {
            router_error!("Could not estimate flight time: {e}.",);

            ItineraryError::Internal
        })?;

        let Ok(flight_window) = Timeslot::new(
            pair.origin_timeslot.time_start(),
            pair.target_timeslot.time_end(),
        ) else {
            router_error!("Could not create flight window.");
            continue;
        };

        let waypoints = Some(GeoLineStringZ {
            points: pair
                .waypoints
                .iter()
                .map(|point| GeoPointZ {
                    y: point.latitude,
                    x: point.longitude,
                    z: point.altitude_meters as f64,
                })
                .collect(),
        });

        for (aircraft_id, availability) in &ordered {
            let flight_plan = svc_storage_client_grpc::prelude::flight_plan::Data {
                origin_vertiport_id: pair.origin_vertiport_id.clone(),
                target_vertiport_id: pair.target_vertiport_id.clone(),
                origin_vertipad_id: pair.origin_vertipad_id.clone(),
                target_vertipad_id: pair.target_vertipad_id.clone(),
                waypoints: waypoints.clone(),
                vehicle_id: aircraft_id.clone(),
                ..Default::default()
            };

            let itinerary = match get_itinerary(
                flight_plan.clone(),
                availability,
                &flight_duration,
                required_loading_time,
                required_unloading_time,
                &flight_window,
                clients,
            )
            .await
            {
                Ok(itinerary) => itinerary,
                Err(ItineraryError::ClientError) => {
                    // exit immediately if svc-gis is down, don't allow new flights
                    router_error!("Could not determine path; client error.");

                    return Err(ItineraryError::ClientError);
                }
                _ => {
                    router_debug!("No itinerary found for aircraft {}.", aircraft_id);
                    continue;
                }
            };

            itineraries.push(itinerary);
            if itineraries.len() >= MAX_ITINERARIES {
                router_info!("max itineraries reached {}.", itineraries.len());

                break 'outer;
            }
        }
    }

    router_info!("found {} itineraries.", itineraries.len());

    Ok(itineraries)
}

/// Struct to hold flight plan metadata
struct DeadheadHelperArgs<'a> {
    origin_vertiport_id: &'a str,
    origin_vertipad_id: &'a str,
    target_vertiport_id: &'a str,
    target_vertipad_id: &'a str,
    vehicle_id: &'a str,
    aircraft_earliest: DateTime<Utc>,
    vertipad_earliest: DateTime<Utc>,
    arrival_latest: DateTime<Utc>,
    required_loading_time: Duration,
    required_unloading_time: Duration,
}

/// Helper function to create a flight plan for a deadhead flight
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need running backend, integration tests
async fn deadhead_helper(
    clients: &GrpcClients,
    args: DeadheadHelperArgs<'_>,
) -> Result<flight_plan::Data, ItineraryError> {
    router_debug!("Deadhead to departure vertiport.");
    // See what the path and cost would be for a flight between the starting
    // available timeslot and the ending flight time
    let best_path_request = BestPathRequest {
        origin_type: NodeType::Vertiport as i32,
        target_type: NodeType::Vertiport as i32,
        origin_identifier: args.origin_vertiport_id.to_owned(),
        target_identifier: args.target_vertiport_id.to_owned(),
        time_start: Some(args.aircraft_earliest.into()),
        time_end: Some(args.vertipad_earliest.into()),
        limit: 1,
    };

    let mut paths = match best_path(&best_path_request, clients).await {
        Ok(paths) => paths,
        Err(BestPathError::NoPathFound) => {
            // no path found, perhaps temporary no-fly zone
            //  is blocking journeys from this depart timeslot
            // Break out and try the next depart timeslot
            router_debug!(
                "No path found from vertiport {}
            to vertiport {} (from {} to {}).",
                best_path_request.origin_identifier,
                best_path_request.target_identifier,
                args.aircraft_earliest,
                args.vertipad_earliest
            );

            return Err(ItineraryError::NoPathFound);
        }
        Err(BestPathError::ClientError) => {
            // exit immediately if svc-gis is down, don't allow new flights
            router_error!("Could not determine path.");
            return Err(ItineraryError::ClientError);
        }
    };

    let (path, distance_meters) = paths.remove(0);
    let points = path
        .iter()
        .filter_map(|node| {
            if node.node_type == NodeType::Vertiport as i32 {
                return None;
            }

            node.geom.map(|geom| GeoPointZ {
                y: geom.latitude,
                x: geom.longitude,
                z: geom.altitude_meters as f64,
            })
        })
        .collect::<Vec<GeoPointZ>>();

    // Leave off the vertiport locations
    let waypoints = Some(GeoLineStringZ { points });

    let flight_duration = estimate_flight_time_seconds(&distance_meters).map_err(|e| {
        router_error!("Could not estimate flight time: {e}");
        ItineraryError::Internal
    })?;

    let total_duration =
        flight_duration + args.required_loading_time + args.required_unloading_time;

    let origin_timeslot_start = max(
        args.aircraft_earliest,
        args.vertipad_earliest - total_duration,
    );

    let origin_timeslot_end = origin_timeslot_start + args.required_loading_time;
    let target_timeslot_start = origin_timeslot_end + flight_duration;
    let target_timeslot_end = target_timeslot_start + args.required_unloading_time;

    let data = flight_plan::Data {
        origin_timeslot_start: Some(origin_timeslot_start.into()),
        origin_timeslot_end: Some(origin_timeslot_end.into()),
        target_timeslot_start: Some(target_timeslot_start.into()),
        target_timeslot_end: Some(target_timeslot_end.into()),
        origin_vertiport_id: args.origin_vertiport_id.to_string(),
        origin_vertipad_id: args.origin_vertipad_id.to_string(),
        target_vertiport_id: args.target_vertiport_id.to_string(),
        target_vertipad_id: args.target_vertipad_id.to_string(),
        vehicle_id: args.vehicle_id.to_string(),
        waypoints,
        ..Default::default()
    };

    if target_timeslot_end > args.arrival_latest {
        // This flight plan would eat into the aircraft's next itinerary
        //  Break out and try the next available timeslot
        router_debug!("Flight plan would end too late.");
        println!("(deadhead_helper) Flight plan would end too late.");
        return Err(ItineraryError::ScheduleConflict);
    }

    Ok(data)
}

/// Determines if the aircraft is available for the requested flight,
///  given that it may require multiple deadhead trips.
async fn get_itinerary(
    flight_plan: flight_plan::Data,
    availability: &Availability,
    flight_duration: &Duration,
    required_loading_time: &Duration,
    required_unloading_time: &Duration,
    flight_window: &Timeslot,
    clients: &GrpcClients,
) -> Result<Vec<flight_plan::Data>, ItineraryError> {
    router_debug!("entry.");

    println!("(get_itinerary) entry.");

    // Must be some overlap between the flight window and the available timeslot
    let Ok(overlap) = availability.timeslot.overlap(flight_window) else {
        router_debug!("No overlap between flight window and available timeslot.");
        return Err(ItineraryError::ScheduleConflict);
    };

    println!("(get_itinerary) overlap: {:#?}", overlap);

    let vehicle_id = flight_plan.vehicle_id.clone();
    let deadhead_loading_time = Duration::zero();

    //
    // 1) Create the flight plan for the deadhead flight to the requested departure vertiport
    //
    let mut flight_plans = vec![];
    if flight_plan.origin_vertiport_id != availability.vertiport_id {
        router_debug!("plotting deadhead to origin.");
        println!("(get_itinerary) plotting deadhead to origin.");

        let args = DeadheadHelperArgs {
            origin_vertiport_id: &availability.vertiport_id,
            origin_vertipad_id: &availability.vertipad_id,
            target_vertiport_id: &flight_plan.origin_vertiport_id,
            target_vertipad_id: &flight_plan.origin_vertipad_id,
            vehicle_id: &vehicle_id,
            aircraft_earliest: availability.timeslot.time_start(),
            vertipad_earliest: overlap.time_start(),
            arrival_latest: overlap.time_end(),
            required_loading_time: deadhead_loading_time, // deadhead - no loading
            required_unloading_time: deadhead_loading_time, // deadhead - no unloading
        };

        let deadhead = match deadhead_helper(clients, args).await {
            Ok(deadhead) => deadhead,
            Err(e) => {
                router_error!("Couldn't schedule deadhead flight: {e}");
                return Err(ItineraryError::ScheduleConflict);
            }
        };

        flight_plans.push(deadhead);
    }

    //
    // 2) Create the flight plan for the requested flight
    //
    router_debug!("plotting primary flight plan.");
    println!("(get_itinerary) plotting primary flight plan.");
    let origin_timeslot_start: DateTime<Utc> = match flight_plans.last() {
        Some(last) => last
            .target_timeslot_end
            .clone()
            .ok_or_else(|| {
                router_error!("Last flight plan has no scheduled target.");
                ItineraryError::Data
            })?
            .into(),
        // leave at earliest possible time
        None => max(
            flight_window.time_start(),
            availability.timeslot.time_start(),
        ),
    };

    let origin_timeslot_end = origin_timeslot_start + *required_loading_time;
    let target_timeslot_start = origin_timeslot_end + *flight_duration;
    let target_timeslot_end = target_timeslot_start + *required_unloading_time;

    if target_timeslot_end > overlap.time_end() {
        // This flight plan would exceed the flight window
        router_debug!("Flight plan would end too late.");
        println!("(get_itinerary) Flight plan would end too late.");
        return Err(ItineraryError::ScheduleConflict);
    }

    // Flight requested by user
    let mut main_flight_plan = flight_plan.clone();
    main_flight_plan.origin_timeslot_start = Some(origin_timeslot_start.into());
    main_flight_plan.origin_timeslot_end = Some(origin_timeslot_end.into());
    main_flight_plan.target_timeslot_start = Some(target_timeslot_start.into());
    main_flight_plan.target_timeslot_end = Some(target_timeslot_end.into());
    flight_plans.push(main_flight_plan.clone());

    //
    // 3) Create the post deadhead flight to take the aircraft away from the pad
    //  when flight is completed
    //
    if flight_plan.target_vertiport_id != availability.vertiport_id {
        router_debug!("plotting deadhead from target.");
        println!("(get_itinerary) plotting deadhead from target.");

        // TODO(R5) - Get nearest open rest stop/hangar, direct to it
        //  right now it boomerangs back to its original last_vertiport_id

        let Some(last_arrival) = &main_flight_plan.target_timeslot_end else {
            router_error!("Last flight plan has no scheduled arrival.");
            return Err(ItineraryError::Data);
        };

        let args = DeadheadHelperArgs {
            origin_vertiport_id: &main_flight_plan.target_vertiport_id,
            origin_vertipad_id: &main_flight_plan.target_vertipad_id,
            target_vertiport_id: &availability.vertiport_id,
            target_vertipad_id: &availability.vertipad_id,
            vehicle_id: &vehicle_id,
            aircraft_earliest: (*last_arrival).clone().into(),
            vertipad_earliest: (*last_arrival).clone().into(), // reserved pad can be accessed any time
            arrival_latest: availability.timeslot.time_end(),
            required_loading_time: deadhead_loading_time, // deadhead - no loading
            required_unloading_time: deadhead_loading_time, // deadhead - no unloading
        };

        let deadhead = match deadhead_helper(clients, args).await {
            Ok(deadhead) => deadhead,
            Err(e) => {
                router_error!("Couldn't schedule deadhead flight: {e}");
                println!("(get_itinerary) Couldn't schedule deadhead flight: {e}");
                return Err(ItineraryError::ScheduleConflict);
            }
        };

        flight_plans.push(deadhead);
    }

    router_debug!("flight_plans: {:#?}", flight_plans);
    Ok(flight_plans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::client::get_clients;
    use lib_common::uuid::Uuid;

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn test_get_itinerary_valid_pre_post_deadheads() {
        let clients = get_clients().await;
        let time_start = Utc::now();
        let time_end = Utc::now() + Duration::try_seconds(1000).unwrap();
        let vertiport_1 = Uuid::new_v4().to_string();
        let vertiport_2 = Uuid::new_v4().to_string();
        let vertiport_3 = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();
        let vehicle_id = Uuid::new_v4().to_string();
        let required_loading_time = Duration::try_seconds(30).unwrap();
        let required_unloading_time = Duration::try_seconds(30).unwrap();

        let aircraft_availability = Availability {
            vertiport_id: vertiport_1.clone(),
            vertipad_id: vertipad_1.clone(),
            timeslot: Timeslot::new(time_start - Duration::try_seconds(1000).unwrap(), time_end)
                .unwrap(),
        };

        let distance_meters = 50.0;
        let flight_duration = estimate_flight_time_seconds(&distance_meters).unwrap();
        let flight_window = Timeslot::new(time_start, time_end).unwrap();

        let flight_plan = flight_plan::Data {
            origin_vertiport_id: vertiport_3.clone(),
            target_vertiport_id: vertiport_2.clone(),
            origin_vertipad_id: vertipad_1.clone(),
            target_vertipad_id: vertipad_2.clone(),
            vehicle_id,
            waypoints: Some(GeoLineStringZ { points: vec![] }),
            ..Default::default()
        };

        let itinerary = get_itinerary(
            flight_plan,
            &aircraft_availability,
            &flight_duration,
            &required_loading_time,
            &required_unloading_time,
            &flight_window,
            &clients,
        )
        .await
        .unwrap();

        // 3 flight plans: deadhead to vertiport_3, flight to vertiport_2, deadhead to vertiport_1
        assert_eq!(itinerary.len(), 3);
        assert_eq!(itinerary[0].origin_vertiport_id.clone(), vertiport_1);
        assert_eq!(itinerary[0].target_vertiport_id.clone(), vertiport_3);
        assert_eq!(itinerary[0].target_vertipad_id.clone(), vertipad_1);

        assert_eq!(itinerary[1].origin_vertiport_id.clone(), vertiport_3);
        assert_eq!(itinerary[1].origin_vertipad_id.clone(), vertipad_1);
        assert_eq!(itinerary[1].target_vertiport_id.clone(), vertiport_2);
        assert_eq!(itinerary[1].target_vertipad_id.clone(), vertipad_2);

        assert_eq!(itinerary[2].origin_vertiport_id.clone(), vertiport_2);
        assert_eq!(itinerary[2].origin_vertipad_id.clone(), vertipad_2);
        assert_eq!(itinerary[2].target_vertiport_id.clone(), vertiport_1);

        // Land at earliest possible time
        assert_eq!(
            // deadhead
            itinerary[0].target_timeslot_start.clone().unwrap(),
            time_start.into() // early as possible in vertipad timeslot
        );
        assert_eq!(
            itinerary[0].target_timeslot_end.clone().unwrap(),
            time_start.into() // no unloading time needed for deadhead
        );

        assert_eq!(
            // main flight
            itinerary[1].origin_timeslot_start.clone().unwrap(),
            time_start.into()
        );

        assert_eq!(
            // main flight
            itinerary[1].target_timeslot_start.clone().unwrap(),
            (time_start + required_loading_time + flight_duration).into()
        );
        assert_eq!(
            // main flight
            itinerary[1].target_timeslot_end.clone().unwrap(),
            (time_start + required_loading_time + flight_duration + required_unloading_time).into()
        );
        assert_eq!(
            // deadhead
            itinerary[2].origin_timeslot_start.clone().unwrap(),
            (time_start + required_loading_time + flight_duration + required_unloading_time).into()
        );
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn test_get_itinerary_valid_pre_deadhead() {
        let clients = get_clients().await;
        let time_start = Utc::now();
        let time_end = Utc::now() + Duration::try_seconds(1000).unwrap();
        let vertiport_1 = Uuid::new_v4().to_string();
        let vertiport_3 = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();
        let vehicle_id = Uuid::new_v4().to_string();
        let required_loading_time = Duration::try_seconds(30).unwrap();
        let required_unloading_time = Duration::try_seconds(30).unwrap();

        let aircraft_availability = Availability {
            vertiport_id: vertiport_1.clone(),
            vertipad_id: vertipad_1.clone(),
            timeslot: Timeslot::new(
                time_start - Duration::try_seconds(1000).unwrap(),
                time_end + Duration::try_seconds(1000).unwrap(),
            )
            .unwrap(),
        };

        let distance_meters = 50.0;
        let flight_duration = estimate_flight_time_seconds(&distance_meters).unwrap();
        let flight_window = Timeslot::new(time_start, time_end).unwrap();

        let flight_plan = flight_plan::Data {
            origin_vertiport_id: vertiport_3.clone(),
            target_vertiport_id: vertiport_1.clone(),
            origin_vertipad_id: vertipad_1.clone(),
            target_vertipad_id: vertipad_2.clone(),
            vehicle_id,
            waypoints: Some(GeoLineStringZ { points: vec![] }),
            ..Default::default()
        };

        let itinerary = get_itinerary(
            flight_plan,
            &aircraft_availability,
            &flight_duration,
            &required_loading_time,
            &required_unloading_time,
            &flight_window,
            &clients,
        )
        .await
        .unwrap();

        // 2 flight plans: deadhead to vertiport_3, flight to vertiport_1
        assert_eq!(itinerary.len(), 2);
        assert_eq!(itinerary[0].origin_vertiport_id.clone(), vertiport_1);
        assert_eq!(itinerary[0].target_vertiport_id.clone(), vertiport_3);
        assert_eq!(itinerary[0].target_vertipad_id.clone(), vertipad_1);

        assert_eq!(itinerary[1].origin_vertiport_id.clone(), vertiport_3);
        assert_eq!(itinerary[1].origin_vertipad_id.clone(), vertipad_1);
        assert_eq!(itinerary[1].target_vertiport_id.clone(), vertiport_1);
        assert_eq!(itinerary[1].target_vertipad_id.clone(), vertipad_2);

        // Land at earliest possible time
        assert_eq!(
            // deadhead flight
            itinerary[0].target_timeslot_end.clone().unwrap(),
            (time_start).into() // deadhead flight should arrive as early as possible to time_start
        );
        assert_eq!(
            itinerary[1].origin_timeslot_start.clone().unwrap(),
            time_start.into()
        );
        assert_eq!(
            itinerary[1].origin_timeslot_end.clone().unwrap(),
            (time_start + required_loading_time).into()
        );
        assert_eq!(
            itinerary[1].target_timeslot_start.clone().unwrap(),
            (time_start + required_loading_time + flight_duration).into()
        );
        assert_eq!(
            itinerary[1].target_timeslot_end.clone().unwrap(),
            (time_start + required_loading_time + flight_duration + required_unloading_time).into()
        );
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn test_get_itinerary_valid_post_deadhead() {
        let clients = get_clients().await;
        let time_start = Utc::now();
        let time_end = Utc::now() + Duration::try_seconds(1000).unwrap();
        let vertiport_1 = Uuid::new_v4().to_string();
        let vertiport_3 = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();
        let vehicle_id = Uuid::new_v4().to_string();
        let required_loading_time = Duration::try_seconds(30).unwrap();
        let required_unloading_time = Duration::try_seconds(30).unwrap();

        let aircraft_availability = Availability {
            vertiport_id: vertiport_1.clone(),
            vertipad_id: vertipad_1.clone(),
            timeslot: Timeslot::new(time_start - Duration::try_seconds(1000).unwrap(), time_end)
                .unwrap(),
        };

        let distance_meters = 50.0;
        let flight_duration = estimate_flight_time_seconds(&distance_meters).unwrap();
        let flight_window = Timeslot::new(time_start, time_end).unwrap();

        let flight_plan = flight_plan::Data {
            origin_vertiport_id: vertiport_1.clone(),
            target_vertiport_id: vertiport_3.clone(),
            origin_vertipad_id: vertipad_1.clone(),
            target_vertipad_id: vertipad_2.clone(),
            vehicle_id,
            waypoints: Some(GeoLineStringZ { points: vec![] }),
            ..Default::default()
        };

        let itinerary = get_itinerary(
            flight_plan,
            &aircraft_availability,
            &flight_duration,
            &required_loading_time,
            &required_unloading_time,
            &flight_window,
            &clients,
        )
        .await
        .unwrap();

        // 2 flight plans: flight to vertiport_3, deadhead to vertiport_1
        assert_eq!(itinerary.len(), 2);
        assert_eq!(itinerary[0].origin_vertiport_id.clone(), vertiport_1);
        assert_eq!(itinerary[0].origin_vertipad_id.clone(), vertipad_1);
        assert_eq!(itinerary[0].target_vertiport_id.clone(), vertiport_3);
        assert_eq!(itinerary[0].target_vertipad_id.clone(), vertipad_2);

        assert_eq!(itinerary[1].origin_vertiport_id.clone(), vertiport_3);
        assert_eq!(itinerary[1].origin_vertipad_id.clone(), vertipad_2);
        assert_eq!(itinerary[1].target_vertiport_id.clone(), vertiport_1);

        // Land at earliest possible time
        assert_eq!(
            itinerary[0].origin_timeslot_start.clone().unwrap(),
            time_start.into()
        );
        assert_eq!(
            itinerary[0].origin_timeslot_end.clone().unwrap(),
            (time_start + required_loading_time).into()
        );
        assert_eq!(
            itinerary[0].target_timeslot_start.clone().unwrap(),
            (time_start + required_loading_time + flight_duration).into()
        );
        assert_eq!(
            itinerary[0].target_timeslot_end.clone().unwrap(),
            (time_start + required_loading_time + flight_duration + required_unloading_time).into()
        );
        assert_eq!(
            itinerary[1].origin_timeslot_start.clone().unwrap(),
            (time_start + required_loading_time + flight_duration + required_unloading_time).into()
        );
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn test_get_itinerary_valid_later_flight_window() {
        let clients = get_clients().await;
        let time_start = Utc::now();
        let time_end = Utc::now() + Duration::try_hours(1).unwrap();
        let vertiport_1 = Uuid::new_v4().to_string();
        let vertiport_2 = Uuid::new_v4().to_string();
        let vertiport_3 = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();
        let vehicle_id = Uuid::new_v4().to_string();
        let required_loading_time = Duration::try_seconds(30).unwrap();
        let required_unloading_time = Duration::try_seconds(30).unwrap();

        //       |    flight window  |
        //  |     takeoff and land time window     |
        //

        let aircraft_availability = Availability {
            vertiport_id: vertiport_1.clone(),
            vertipad_id: vertipad_1.clone(),
            timeslot: Timeslot::new(
                time_start + Duration::try_minutes(10).unwrap(),
                time_end - Duration::try_minutes(20).unwrap(),
            )
            .unwrap(),
        };

        let distance_meters = 50.0;
        let flight_duration = estimate_flight_time_seconds(&distance_meters).unwrap();
        let flight_window = Timeslot::new(time_start, time_end).unwrap();

        let flight_plan = flight_plan::Data {
            origin_vertiport_id: vertiport_3.clone(),
            target_vertiport_id: vertiport_2.clone(),
            origin_vertipad_id: vertipad_1.clone(),
            target_vertipad_id: vertipad_2.clone(),
            vehicle_id,
            waypoints: Some(GeoLineStringZ { points: vec![] }),
            ..Default::default()
        };

        let itinerary = get_itinerary(
            flight_plan,
            &aircraft_availability,
            &flight_duration,
            &required_loading_time,
            &required_unloading_time,
            &flight_window,
            &clients,
        )
        .await
        .unwrap();

        // 3 flight plans: deadhead to vertiport_3, flight to vertiport_2, deadhead to vertiport_1
        assert_eq!(itinerary.len(), 3);
        assert_eq!(itinerary[0].origin_vertiport_id.clone(), vertiport_1);
        assert_eq!(itinerary[0].target_vertiport_id.clone(), vertiport_3);
        assert_eq!(itinerary[0].target_vertipad_id.clone(), vertipad_1);

        assert_eq!(itinerary[1].origin_vertiport_id.clone(), vertiport_3);
        assert_eq!(itinerary[1].origin_vertipad_id.clone(), vertipad_1);
        assert_eq!(itinerary[1].target_vertiport_id.clone(), vertiport_2);
        assert_eq!(itinerary[1].target_vertipad_id.clone(), vertipad_2);

        assert_eq!(itinerary[2].origin_vertiport_id.clone(), vertiport_2);
        assert_eq!(itinerary[2].origin_vertipad_id.clone(), vertipad_2);
        assert_eq!(itinerary[2].target_vertiport_id.clone(), vertiport_1);

        // First itinerary for aircraft leaves at earliest aircraft convenience
        assert_eq!(
            itinerary[0].origin_timeslot_start.clone().unwrap(),
            aircraft_availability.timeslot.time_start().into()
        );
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn test_get_itinerary_valid_incompatible_flight_window() {
        let clients = get_clients().await;
        let time_start = Utc::now();
        let time_end = Utc::now() + Duration::try_hours(1).unwrap();
        let vertiport_1 = Uuid::new_v4().to_string();
        let vertiport_2 = Uuid::new_v4().to_string();
        let vertiport_3 = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();
        let vehicle_id = Uuid::new_v4().to_string();
        let required_loading_time = Duration::try_seconds(30).unwrap();
        let required_unloading_time = Duration::try_seconds(30).unwrap();

        //                                       |    flight window    |
        //  |     takeoff and land time window     |
        //

        let aircraft_availability = Availability {
            vertiport_id: vertiport_1.clone(),
            vertipad_id: vertipad_1.clone(),
            timeslot: Timeslot::new(
                time_end - Duration::try_seconds(30).unwrap(),
                time_end + Duration::try_minutes(20).unwrap(),
            )
            .unwrap(),
        };

        let distance_meters = 1000.0; // too far to fly
        let flight_duration = estimate_flight_time_seconds(&distance_meters).unwrap();
        let flight_window = Timeslot::new(time_start, time_end).unwrap();

        let flight_plan = flight_plan::Data {
            origin_vertiport_id: vertiport_3.clone(),
            target_vertiport_id: vertiport_2.clone(),
            origin_vertipad_id: vertipad_1.clone(),
            target_vertipad_id: vertipad_2.clone(),
            vehicle_id,
            waypoints: Some(GeoLineStringZ { points: vec![] }),
            ..Default::default()
        };

        let e = get_itinerary(
            flight_plan,
            &aircraft_availability,
            &flight_duration,
            &required_loading_time,
            &required_unloading_time,
            &flight_window,
            &clients,
        )
        .await
        .unwrap_err();
        assert_eq!(e, ItineraryError::ScheduleConflict);
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn test_calculate_itineraries() {
        let clients = get_clients().await;
        let time_start = Utc::now();
        let time_end = Utc::now() + Duration::try_seconds(1000).unwrap();
        let vertiport_1 = Uuid::new_v4().to_string();
        let vertiport_2 = Uuid::new_v4().to_string();
        let vertiport_3 = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_3 = Uuid::new_v4().to_string();
        let vehicle_1 = Uuid::new_v4().to_string();
        let vehicle_2 = Uuid::new_v4().to_string();
        let required_loading_time = Duration::try_seconds(30).unwrap();
        let required_unloading_time = Duration::try_seconds(30).unwrap();

        let availabilities = HashMap::from([
            (
                vehicle_1.clone(),
                vec![Availability {
                    vertiport_id: vertiport_1.clone(),
                    vertipad_id: vertipad_1.clone(),
                    timeslot: Timeslot::new(
                        time_start - Duration::try_hours(1).unwrap(),
                        time_end + Duration::try_hours(1).unwrap(),
                    )
                    .unwrap(),
                }],
            ),
            (
                vehicle_2.clone(),
                vec![Availability {
                    vertiport_id: vertiport_3.clone(),
                    vertipad_id: vertipad_3.clone(),
                    timeslot: Timeslot::new(
                        time_end + Duration::try_hours(1).unwrap(),
                        time_end + Duration::try_hours(2).unwrap(),
                    )
                    .unwrap(),
                }],
            ),
        ]);

        let distance_meters = 50.0;
        let flight_duration = estimate_flight_time_seconds(&distance_meters).unwrap();
        let timeslot_pairs = vec![
            TimeslotPair {
                origin_vertiport_id: vertiport_1.clone(),
                origin_vertipad_id: vertipad_1.clone(),
                origin_timeslot: Timeslot::new(time_start.clone(), time_end.clone()).unwrap(),
                target_vertiport_id: vertiport_2.clone(),
                target_vertipad_id: vertiport_2.clone(),
                target_timeslot: Timeslot::new(
                    time_start + flight_duration,
                    time_end + flight_duration,
                )
                .unwrap(),
                waypoints: vec![],
                distance_meters,
            },
            TimeslotPair {
                origin_vertiport_id: vertiport_1.clone(),
                origin_vertipad_id: vertipad_1.clone(),
                origin_timeslot: Timeslot::new(
                    time_end + Duration::try_hours(1).unwrap(),
                    time_end + Duration::try_hours(2).unwrap(),
                )
                .unwrap(),
                target_vertiport_id: vertiport_2.clone(),
                target_vertipad_id: vertiport_2.clone(),
                target_timeslot: Timeslot::new(
                    time_end + Duration::try_hours(1).unwrap() + flight_duration,
                    time_end + Duration::try_hours(2).unwrap() + flight_duration,
                )
                .unwrap(),
                waypoints: vec![],
                distance_meters,
            },
        ];

        let itineraries = calculate_itineraries(
            &required_loading_time,
            &required_unloading_time,
            &timeslot_pairs,
            &availabilities,
            &clients,
        )
        .await
        .unwrap();

        // Expect two matches
        println!("{:?}", itineraries);
        for (i, itinerary) in itineraries.iter().enumerate() {
            println!("\n\n----- Itinerary {}", i);
            for (fp_i, fp) in itinerary.iter().enumerate() {
                println!("{}: {:?}\n", fp_i, fp);
            }
        }

        assert_eq!(itineraries.len(), 2);
    }

    #[test]
    fn test_validate_itinerary_not_enough_flight_plans() {
        let mut vertipad_ids = HashSet::<String>::new();
        let mut aircraft_id = String::new();
        let e = validate_itinerary(&vec![], &mut vertipad_ids, &mut aircraft_id).unwrap_err();
        assert_eq!(e, ItineraryError::Data);

        let vehicle_id = Uuid::new_v4().to_string();
        let _ = validate_itinerary(
            &vec![FlightPlanSchedule {
                origin_vertiport_id: Uuid::new_v4().to_string(),
                origin_vertipad_id: Uuid::new_v4().to_string(),
                origin_timeslot_start: Utc::now() + Duration::try_minutes(10).unwrap(),
                origin_timeslot_end: Utc::now() + Duration::try_minutes(11).unwrap(),
                target_vertiport_id: Uuid::new_v4().to_string(),
                target_vertipad_id: Uuid::new_v4().to_string(),
                target_timeslot_start: Utc::now() + Duration::try_minutes(30).unwrap(),
                target_timeslot_end: Utc::now() + Duration::try_minutes(31).unwrap(),
                vehicle_id: vehicle_id.clone(),
                waypoints: Some(vec![]),
            }],
            &mut vertipad_ids,
            &mut aircraft_id,
        )
        .unwrap();

        assert_eq!(vertipad_ids.len(), 2);
        assert_eq!(aircraft_id, vehicle_id);
    }

    #[test]
    fn test_validate_itinerary_inconsistent_aircraft() -> Result<(), ItineraryError> {
        let mut vertipad_ids = HashSet::<String>::new();
        let mut aircraft_id = String::new();

        let vehicle_id = Uuid::new_v4();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();

        let flight_plans = vec![
            FlightPlanSchedule {
                origin_vertiport_id: Uuid::new_v4().to_string(),
                origin_vertipad_id: vertipad_1.clone(),
                origin_timeslot_start: Utc::now() + Duration::try_minutes(10).unwrap(),
                origin_timeslot_end: Utc::now() + Duration::try_minutes(11).unwrap(),
                target_vertiport_id: Uuid::new_v4().to_string(),
                target_vertipad_id: vertipad_2.clone(),
                target_timeslot_start: Utc::now() + Duration::try_minutes(30).unwrap(),
                target_timeslot_end: Utc::now() + Duration::try_minutes(31).unwrap(),
                vehicle_id: vehicle_id.clone().to_string(),
                waypoints: Some(vec![]),
            },
            FlightPlanSchedule {
                origin_vertiport_id: Uuid::new_v4().to_string(),
                origin_vertipad_id: vertipad_2.clone(),
                origin_timeslot_start: Utc::now() + Duration::try_minutes(10).unwrap(),
                origin_timeslot_end: Utc::now() + Duration::try_minutes(11).unwrap(),
                target_vertiport_id: Uuid::new_v4().to_string(),
                target_vertipad_id: Uuid::new_v4().to_string(),
                target_timeslot_start: Utc::now() + Duration::try_minutes(30).unwrap(),
                target_timeslot_end: Utc::now() + Duration::try_minutes(31).unwrap(),
                vehicle_id: Uuid::new_v4().to_string(),
                waypoints: Some(vec![]),
            },
        ];

        let e = validate_itinerary(&flight_plans, &mut vertipad_ids, &mut aircraft_id).unwrap_err();
        assert_eq!(e, ItineraryError::VehicleId);

        Ok(())
    }

    #[test]
    fn test_validate_itinerary_invalid_paths() -> Result<(), ItineraryError> {
        let mut vertipad_ids = HashSet::<String>::new();
        let mut aircraft_id = String::new();

        let vehicle_id = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();

        let mut flight_plans = vec![
            FlightPlanSchedule {
                origin_vertiport_id: Uuid::new_v4().to_string(),
                origin_vertipad_id: vertipad_1.clone(),
                origin_timeslot_start: Utc::now() + Duration::try_minutes(10).unwrap(),
                origin_timeslot_end: Utc::now() + Duration::try_minutes(11).unwrap(),
                target_vertiport_id: Uuid::new_v4().to_string(),
                target_vertipad_id: vertipad_2.clone(),
                target_timeslot_start: Utc::now() + Duration::try_minutes(30).unwrap(),
                target_timeslot_end: Utc::now() + Duration::try_minutes(31).unwrap(),
                vehicle_id: vehicle_id.clone(),
                waypoints: None,
            },
            FlightPlanSchedule {
                origin_vertiport_id: Uuid::new_v4().to_string(),
                origin_vertipad_id: vertipad_2.clone(),
                origin_timeslot_start: Utc::now() + Duration::try_minutes(31).unwrap(),
                origin_timeslot_end: Utc::now() + Duration::try_minutes(32).unwrap(),
                target_vertiport_id: Uuid::new_v4().to_string(),
                target_vertipad_id: Uuid::new_v4().to_string(),
                target_timeslot_start: Utc::now() + Duration::try_minutes(33).unwrap(),
                target_timeslot_end: Utc::now() + Duration::try_minutes(34).unwrap(),
                vehicle_id,
                waypoints: None,
            },
        ];

        let e = validate_itinerary(&flight_plans, &mut vertipad_ids, &mut aircraft_id).unwrap_err();
        assert_eq!(e, ItineraryError::NoPath);

        flight_plans[0].waypoints = Some(vec![]);
        let e = validate_itinerary(&flight_plans, &mut vertipad_ids, &mut aircraft_id).unwrap_err();
        assert_eq!(e, ItineraryError::PathTooShort);

        flight_plans[0].waypoints = Some(vec![PointZ {
            latitude: 0.0,
            longitude: 0.0,
            altitude_meters: 0.0,
        }]);
        let e = validate_itinerary(&flight_plans, &mut vertipad_ids, &mut aircraft_id).unwrap_err();
        assert_eq!(e, ItineraryError::PathTooShort);

        Ok(())
    }

    #[test]
    fn test_validate_itinerary_inconsistent_vertipads() -> Result<(), ItineraryError> {
        let mut vertipad_ids = HashSet::<String>::new();
        let mut aircraft_id = String::new();

        let vehicle_id = Uuid::new_v4();
        let flight_plans = vec![
            FlightPlanSchedule {
                origin_vertiport_id: Uuid::new_v4().to_string(),
                origin_vertipad_id: Uuid::new_v4().to_string(),
                origin_timeslot_start: Utc::now() + Duration::try_minutes(10).unwrap(),
                origin_timeslot_end: Utc::now() + Duration::try_minutes(11).unwrap(),
                target_vertiport_id: Uuid::new_v4().to_string(),
                target_vertipad_id: Uuid::new_v4().to_string(),
                target_timeslot_start: Utc::now() + Duration::try_minutes(30).unwrap(),
                target_timeslot_end: Utc::now() + Duration::try_minutes(31).unwrap(),
                vehicle_id: vehicle_id.clone().to_string(),
                waypoints: Some(vec![]),
            },
            FlightPlanSchedule {
                origin_vertiport_id: Uuid::new_v4().to_string(),
                origin_vertipad_id: Uuid::new_v4().to_string(),
                origin_timeslot_start: Utc::now() + Duration::try_minutes(10).unwrap(),
                origin_timeslot_end: Utc::now() + Duration::try_minutes(11).unwrap(),
                target_vertiport_id: Uuid::new_v4().to_string(),
                target_vertipad_id: Uuid::new_v4().to_string(),
                target_timeslot_start: Utc::now() + Duration::try_minutes(30).unwrap(),
                target_timeslot_end: Utc::now() + Duration::try_minutes(31).unwrap(),
                vehicle_id: vehicle_id.clone().to_string(),
                waypoints: Some(vec![]),
            },
        ];

        let e = validate_itinerary(&flight_plans, &mut vertipad_ids, &mut aircraft_id).unwrap_err();
        assert_eq!(e, ItineraryError::Vertipads);

        Ok(())
    }

    #[test]
    fn test_validate_itinerary_invalid_times() -> Result<(), ItineraryError> {
        let mut vertipad_ids = HashSet::<String>::new();
        let mut aircraft_id = String::new();

        let vehicle_id = Uuid::new_v4();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();

        let vertiport_2 = Uuid::new_v4().to_string();
        let flight_plans = vec![
            FlightPlanSchedule {
                origin_vertiport_id: Uuid::new_v4().to_string(),
                origin_vertipad_id: vertipad_1.clone(),
                origin_timeslot_start: Utc::now() + Duration::try_minutes(10).unwrap(),
                origin_timeslot_end: Utc::now() + Duration::try_minutes(11).unwrap(),
                target_vertiport_id: vertiport_2.clone(),
                target_vertipad_id: vertipad_2.clone(),
                target_timeslot_start: Utc::now() + Duration::try_minutes(30).unwrap(),
                target_timeslot_end: Utc::now() + Duration::try_minutes(31).unwrap(),
                vehicle_id: vehicle_id.clone().to_string(),
                waypoints: Some(vec![]),
            },
            FlightPlanSchedule {
                origin_vertiport_id: vertiport_2,
                origin_vertipad_id: vertipad_2.clone(),
                origin_timeslot_start: Utc::now() + Duration::try_minutes(10).unwrap(),
                origin_timeslot_end: Utc::now() + Duration::try_minutes(11).unwrap(),
                target_vertiport_id: Uuid::new_v4().to_string(),
                target_vertipad_id: Uuid::new_v4().to_string(),
                target_timeslot_start: Utc::now() + Duration::try_minutes(40).unwrap(),
                target_timeslot_end: Utc::now() + Duration::try_minutes(41).unwrap(),
                vehicle_id: vehicle_id.clone().to_string(),
                waypoints: Some(vec![]),
            },
        ];

        let e = validate_itinerary(&flight_plans, &mut vertipad_ids, &mut aircraft_id).unwrap_err();
        assert_eq!(e, ItineraryError::TimeWindow);
        Ok(())
    }

    #[test]
    fn test_validate_itinerary_updated_aircraft_and_vertipads() -> Result<(), ItineraryError> {
        let mut vertipad_ids = HashSet::<String>::new();
        let mut aircraft_id = String::new();

        let vehicle_id = Uuid::new_v4();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();

        let waypoints = Some(vec![
            PointZ {
                latitude: 0.0,
                longitude: 0.0,
                altitude_meters: 20.0,
            },
            PointZ {
                latitude: 0.0,
                longitude: 0.0,
                altitude_meters: 20.0,
            },
        ]);

        let flight_plans = vec![
            FlightPlanSchedule {
                origin_vertiport_id: Uuid::new_v4().to_string(),
                origin_vertipad_id: vertipad_1.clone(),
                origin_timeslot_start: Utc::now() + Duration::try_minutes(10).unwrap(),
                origin_timeslot_end: Utc::now() + Duration::try_minutes(11).unwrap(),
                target_vertiport_id: Uuid::new_v4().to_string(),
                target_vertipad_id: vertipad_2.clone(),
                target_timeslot_start: Utc::now() + Duration::try_minutes(30).unwrap(),
                target_timeslot_end: Utc::now() + Duration::try_minutes(31).unwrap(),
                vehicle_id: vehicle_id.clone().to_string(),
                waypoints: waypoints.clone(),
            },
            FlightPlanSchedule {
                origin_vertiport_id: Uuid::new_v4().to_string(),
                origin_vertipad_id: vertipad_2.clone(),
                origin_timeslot_start: Utc::now() + Duration::try_minutes(31).unwrap(),
                origin_timeslot_end: Utc::now() + Duration::try_minutes(32).unwrap(),
                target_vertiport_id: Uuid::new_v4().to_string(),
                target_vertipad_id: Uuid::new_v4().to_string(),
                target_timeslot_start: Utc::now() + Duration::try_minutes(40).unwrap(),
                target_timeslot_end: Utc::now() + Duration::try_minutes(41).unwrap(),
                vehicle_id: vehicle_id.clone().to_string(),
                waypoints,
            },
        ];

        validate_itinerary(&flight_plans, &mut vertipad_ids, &mut aircraft_id)?;
        assert_eq!(vehicle_id.to_string(), aircraft_id);

        assert!(vertipad_ids.contains(&flight_plans[0].origin_vertipad_id));
        assert!(vertipad_ids.contains(&flight_plans[0].target_vertipad_id));
        assert!(vertipad_ids.contains(&flight_plans[1].origin_vertipad_id));
        assert!(vertipad_ids.contains(&flight_plans[1].target_vertipad_id));

        Ok(())
    }

    #[test]
    fn test_itinerary_error_display() {
        assert_eq!(
            ItineraryError::ClientError.to_string(),
            "Could not contact dependency."
        );
        assert_eq!(ItineraryError::Data.to_string(), "Invalid data.");
        assert_eq!(
            ItineraryError::VehicleId.to_string(),
            "Inconsistent vehicle ID."
        );
        assert_eq!(ItineraryError::NoPath.to_string(), "No path provided.");
        assert_eq!(
            ItineraryError::PathTooShort.to_string(),
            "Path is too short."
        );
        assert_eq!(
            ItineraryError::TimeWindow.to_string(),
            "Invalid time window."
        );
        assert_eq!(
            ItineraryError::Vertipads.to_string(),
            "Inconsistent vertipads."
        );
        assert_eq!(
            ItineraryError::ScheduleConflict.to_string(),
            "Schedule conflict."
        );
        assert_eq!(ItineraryError::Internal.to_string(), "Internal error.");
        assert_eq!(ItineraryError::NoPathFound.to_string(), "No path found.");
    }
}
