//! Build an itinerary given aircraft availability and the flight window

use super::flight_plan::FlightPlanSchedule;
use super::schedule::*;
use super::vehicle::*;
use super::vertiport::TimeslotPair;
use super::{best_path, BestPathError, BestPathRequest};
use crate::grpc::client::GrpcClients;
use svc_gis_client_grpc::prelude::gis::*;
use svc_storage_client_grpc::prelude::*;

use chrono::{DateTime, Duration, Utc};
use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter, Result as FmtResult};

/// Errors that may occur while processing an itinerary
#[derive(Debug, Clone, PartialEq)]
pub enum ItineraryError {
    /// There was an error contacting a dependency
    ClientError,

    /// The provided data was invalid
    InvalidData,

    /// No path could be found between the origin and target vertipads
    NoPathFound,

    /// There was a schedule conflict
    ScheduleConflict,
}

impl Display for ItineraryError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            ItineraryError::ClientError => write!(f, "Could not contact dependency."),
            ItineraryError::InvalidData => write!(f, "Invalid data."),
            ItineraryError::NoPathFound => write!(f, "No path found."),
            ItineraryError::ScheduleConflict => write!(f, "Schedule conflict."),
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
        router_error!("(validate_itinerary) No flight plans provided.");
        return Err(ItineraryError::InvalidData);
    }

    if flight_plans.len() == 1 {
        router_debug!("(validate_itinerary) Only one flight plan provided.");
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
                "(validate_itinerary) {error_msg}: {} -> {}",
                fp_1.target_vertipad_id,
                fp_2.origin_vertipad_id
            );

            return Err(ItineraryError::InvalidData);
        }

        vertipad_ids.insert(fp_1.origin_vertipad_id.clone());
        vertipad_ids.insert(fp_1.target_vertipad_id.clone());
        vertipad_ids.insert(fp_2.target_vertipad_id.clone());

        if aircraft_id.is_empty() {
            *aircraft_id = fp_1.vehicle_id.clone();
        }

        if fp_1.vehicle_id != fp_2.vehicle_id {
            router_error!(
                "(validate_itinerary) Flight plans should use the same aircraft: {:#?}",
                flight_plans
            );

            return Err(ItineraryError::InvalidData);
        }

        if *aircraft_id != fp_2.vehicle_id {
            router_error!(
                "(validate_itinerary) Flight plans should use the same aircraft: {:#?}",
                flight_plans
            );

            return Err(ItineraryError::InvalidData);
        }

        if fp_1.origin_timeslot_start >= fp_1.target_timeslot_start {
            router_error!(
                "(validate_itinerary) Flight plans should be in order of departure time: {:#?}",
                flight_plans
            );

            return Err(ItineraryError::InvalidData);
        }

        if fp_1.target_timeslot_end >= fp_2.origin_timeslot_start {
            router_error!(
                "(validate_itinerary) Flight plans should be in order of departure time: {:#?}",
                flight_plans
            );

            return Err(ItineraryError::InvalidData);
        }
    }

    Ok(())
}

/// Given timeslot pairs for departure and arrival vertiport and the
///  availabilities of the aircraft, get possible itineraries for each
///  aircraft.
/// Returns a maximum of 1 itinerary per aircraft.
pub async fn calculate_itineraries(
    required_loading_time: &Duration,
    required_unloading_time: &Duration,
    timeslot_pairs: &[TimeslotPair],
    aircraft_gaps: &HashMap<String, Vec<Availability>>,
    clients: &GrpcClients,
) -> Result<Vec<Vec<flight_plan::Data>>, ItineraryError> {
    let mut itineraries: Vec<Vec<flight_plan::Data>> = vec![];

    router_debug!(
        "(calculate_itineraries) aircraft_gaps: {:#?}",
        aircraft_gaps
    );
    router_debug!(
        "(calculate_itineraries) timeslot_pairs: {:#?}",
        timeslot_pairs
    );

    // For each available aircraft, see if it can do the flight
    for (aircraft_id, aircraft_availability) in aircraft_gaps {
        // Try different timeslots for the aircraft
        for pair in timeslot_pairs {
            // TODO(R4): Include vehicle model to improve estimate
            let flight_duration = estimate_flight_time_seconds(&pair.distance_meters);

            let flight_window = Timeslot {
                time_start: pair.origin_timeslot.time_start,
                time_end: pair.target_timeslot.time_end,
            };

            let flight_plan = svc_storage_client_grpc::prelude::flight_plan::Data {
                origin_vertiport_id: Some(pair.origin_port_id.clone()),
                target_vertiport_id: Some(pair.target_port_id.clone()),
                origin_vertipad_id: pair.origin_pad_id.clone(),
                target_vertipad_id: pair.target_pad_id.clone(),
                path: Some(pair.path.clone()),
                vehicle_id: aircraft_id.clone(),
                ..Default::default()
            };

            match aircraft_selection(
                flight_plan,
                aircraft_availability,
                &flight_duration,
                required_loading_time,
                required_unloading_time,
                &flight_window,
                clients,
            )
            .await
            {
                Ok(itinerary) => itineraries.push(itinerary),
                Err(ItineraryError::ClientError) => {
                    // exit immediately if svc-gis is down, don't allow new flights
                    router_error!(
                        "(calculate_itineraries) Could not determine path; client error."
                    );
                    return Err(ItineraryError::ClientError);
                }
                _ => {
                    router_debug!(
                        "(calculate_itineraries) No itinerary found for aircraft {}.",
                        aircraft_id
                    );
                    continue;
                }
            }
        }
    }

    router_info!(
        "(calculate_itineraries) found {} itineraries.",
        itineraries.len()
    );

    Ok(itineraries)
}

/// Iterate through an aircraft's available timeslots
///  and see if it can do the requested flight.
/// TODO(R4): Return more than one itinerary per aircraft
async fn aircraft_selection(
    flight_plan: flight_plan::Data,
    availability: &[Availability],
    flight_duration: &Duration,
    required_loading_time: &Duration,
    required_unloading_time: &Duration,
    flight_window: &Timeslot,
    clients: &GrpcClients,
) -> Result<Vec<flight_plan::Data>, ItineraryError> {
    router_debug!("(aircraft_selection) availabilities: {:#?}", availability);

    for gap in availability.iter() {
        match get_itinerary(
            flight_plan.clone(),
            gap,
            flight_duration,
            required_loading_time,
            required_unloading_time,
            flight_window,
            clients,
        )
        .await
        {
            Ok(itinerary) => {
                // only return the first valid itinerary for an aircraft
                return Ok(itinerary);
            }
            Err(ItineraryError::ClientError) => {
                // exit immediately if svc-gis is down, don't allow new flights
                router_error!("(aircraft_selection) Could not determine path; client error.");
                return Err(ItineraryError::ClientError);
            }
            _ => {
                router_debug!(
                    "(aircraft_selection) No itinerary found for aircraft {}.",
                    flight_plan.vehicle_id
                );
                continue;
            }
        }
    }

    Err(ItineraryError::ScheduleConflict)
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
async fn deadhead_helper(
    clients: &GrpcClients,
    args: DeadheadHelperArgs<'_>,
) -> Result<flight_plan::Data, ItineraryError> {
    router_debug!("(deadhead_helper) Deadhead to departure vertiport.");
    // See what the path and cost would be for a flight between the starting
    // available timeslot and the ending flight time
    let best_path_request = BestPathRequest {
        start_type: NodeType::Vertiport as i32,
        node_start_id: args.origin_vertiport_id.to_owned(),
        node_uuid_end: args.target_vertiport_id.to_owned(),
        time_start: Some(args.aircraft_earliest.into()),
        time_end: Some(args.vertipad_earliest.into()),
    };

    let (path, distance_meters) = match best_path(&best_path_request, clients).await {
        Ok((path, d)) => (path, d as f32),
        Err(BestPathError::NoPathFound) => {
            // no path found, perhaps temporary no-fly zone
            //  is blocking journeys from this depart timeslot
            // Break out and try the next depart timeslot
            router_debug!(
                "(deadhead_helper) No path found from vertiport {}
            to vertiport {} (from {} to {}).",
                best_path_request.node_start_id,
                best_path_request.node_uuid_end,
                args.aircraft_earliest,
                args.vertipad_earliest
            );

            return Err(ItineraryError::NoPathFound);
        }
        Err(BestPathError::ClientError) => {
            // exit immediately if svc-gis is down, don't allow new flights
            router_error!("(deadhead_helper) Could not determine path.");
            return Err(ItineraryError::ClientError);
        }
    };

    let flight_duration = estimate_flight_time_seconds(&distance_meters);
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
        origin_vertiport_id: Some(args.origin_vertiport_id.to_string()),
        origin_vertipad_id: args.origin_vertipad_id.to_string(),
        target_vertiport_id: Some(args.target_vertiport_id.to_string()),
        target_vertipad_id: args.target_vertipad_id.to_string(),
        vehicle_id: args.vehicle_id.to_string(),
        path: Some(path),
        ..Default::default()
    };

    if target_timeslot_end > args.arrival_latest {
        // This flight plan would eat into the aircraft's next itinerary
        //  Break out and try the next available timeslot
        router_debug!("(deadhead_helper) Flight plan would end too late.");
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
    router_debug!("(get_itinerary) entry.");

    println!("(get_itinerary) entry.");

    // Must be some overlap between the flight window and the available timeslot
    let Ok(overlap) = availability.timeslot.overlap(flight_window) else {
        router_debug!("(get_itinerary) No overlap between flight window and available timeslot.");
        return Err(ItineraryError::ScheduleConflict);
    };

    println!("(get_itinerary) overlap: {:#?}", overlap);

    let Some(ref origin_vertiport_id) = flight_plan.origin_vertiport_id else {
        router_error!("(get_itinerary) Flight plan doesn't have origin_vertiport_id.",);
        return Err(ItineraryError::InvalidData);
    };

    let Some(ref target_vertiport_id) = flight_plan.target_vertiport_id else {
        router_error!("(get_itinerary) Flight plan doesn't have target_vertiport_id.",);
        return Err(ItineraryError::InvalidData);
    };

    let vehicle_id = flight_plan.vehicle_id.clone();

    //
    // 1) Create the flight plan for the deadhead flight to the requested departure vertiport
    //
    let mut flight_plans = vec![];
    if *origin_vertiport_id != availability.vertiport_id {
        router_debug!("(get_itinerary) plotting deadhead to origin.");
        println!("(get_itinerary) plotting deadhead to origin.");

        let args = DeadheadHelperArgs {
            origin_vertiport_id: &availability.vertiport_id,
            origin_vertipad_id: &availability.vertipad_id,
            target_vertiport_id: origin_vertiport_id,
            target_vertipad_id: &flight_plan.origin_vertipad_id,
            vehicle_id: &vehicle_id,
            aircraft_earliest: availability.timeslot.time_start,
            vertipad_earliest: overlap.time_start,
            arrival_latest: overlap.time_end,
            required_loading_time: Duration::seconds(0), // deadhead - no loading
            required_unloading_time: Duration::seconds(0), // deadhead - no unloading
        };

        let deadhead = match deadhead_helper(clients, args).await {
            Ok(deadhead) => deadhead,
            Err(e) => {
                router_error!("(get_itinerary) Couldn't schedule deadhead flight: {e}");
                println!("(get_itinerary) Couldn't schedule deadhead flight: {e}");
                return Err(ItineraryError::ScheduleConflict);
            }
        };

        flight_plans.push(deadhead);
    }

    //
    // 2) Create the flight plan for the requested flight
    //
    router_debug!("(get_itinerary) plotting primary flight plan.");
    println!("(get_itinerary) plotting primary flight plan.");
    let origin_timeslot_start: DateTime<Utc> = match flight_plans.last() {
        Some(last) => match &last.target_timeslot_end {
            Some(s) => s.clone().into(),
            None => {
                router_error!("(get_itinerary) Last flight plan has no scheduled target.");

                return Err(ItineraryError::InvalidData);
            }
        },
        // leave at earliest possible time
        None => max(flight_window.time_start, availability.timeslot.time_start),
    };

    let origin_timeslot_end = origin_timeslot_start + *required_loading_time;
    let target_timeslot_start = origin_timeslot_end + *flight_duration;
    let target_timeslot_end = target_timeslot_start + *required_unloading_time;

    if target_timeslot_end > overlap.time_end {
        // This flight plan would exceed the flight window
        router_debug!("(get_itinerary) Flight plan would end too late.");
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
    if *target_vertiport_id != availability.vertiport_id {
        router_debug!("(get_itinerary) plotting deadhead from target.");
        println!("(get_itinerary) plotting deadhead from target.");

        // TODO(R4) - Get nearest open rest stop/hangar, direct to it
        //  right now it boomerangs back to its original last_vertiport_id

        let Some(last_arrival) = &main_flight_plan.target_timeslot_end else {
            router_error!("(get_itinerary) Last flight plan has no scheduled arrival.");
            return Err(ItineraryError::InvalidData);
        };

        let Some(target_vertiport_id) = &main_flight_plan.target_vertiport_id else {
            router_error!("(get_itinerary) Last flight plan has no target vertiport.");
            return Err(ItineraryError::InvalidData);
        };

        let args = DeadheadHelperArgs {
            origin_vertiport_id: target_vertiport_id,
            origin_vertipad_id: &main_flight_plan.target_vertipad_id,
            target_vertiport_id: &availability.vertiport_id,
            target_vertipad_id: &availability.vertipad_id,
            vehicle_id: &vehicle_id,
            aircraft_earliest: (*last_arrival).clone().into(),
            vertipad_earliest: (*last_arrival).clone().into(), // reserved pad can be accessed any time
            arrival_latest: availability.timeslot.time_end,
            required_loading_time: Duration::seconds(0), // deadhead - no loading
            required_unloading_time: Duration::seconds(0), // deadhead - no unloading
        };

        let deadhead = match deadhead_helper(clients, args).await {
            Ok(deadhead) => deadhead,
            Err(e) => {
                router_error!("(get_itinerary) Couldn't schedule deadhead flight: {e}");
                println!("(get_itinerary) Couldn't schedule deadhead flight: {e}");
                return Err(ItineraryError::ScheduleConflict);
            }
        };

        flight_plans.push(deadhead);
    }

    router_debug!("(get_itinerary) flight_plans: {:#?}", flight_plans);
    Ok(flight_plans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::client::get_clients;
    use uuid::Uuid;

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn test_get_itinerary_valid_pre_post_deadheads() {
        let clients = get_clients().await;
        let time_start = Utc::now();
        let time_end = Utc::now() + Duration::seconds(1000);
        let vertiport_1 = Uuid::new_v4().to_string();
        let vertiport_2 = Uuid::new_v4().to_string();
        let vertiport_3 = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();
        let vehicle_id = Uuid::new_v4().to_string();
        let required_loading_time = Duration::seconds(30);
        let required_unloading_time = Duration::seconds(30);

        let aircraft_availability = Availability {
            vertiport_id: vertiport_1.clone(),
            vertipad_id: vertipad_1.clone(),
            timeslot: Timeslot {
                time_start: time_start - Duration::seconds(1000),
                time_end: time_end,
            },
        };

        let distance_meters = 50.0;
        let flight_duration = estimate_flight_time_seconds(&distance_meters);
        let flight_window = Timeslot {
            time_end,
            time_start,
        };

        let flight_plan = flight_plan::Data {
            origin_vertiport_id: Some(vertiport_3.clone()),
            target_vertiport_id: Some(vertiport_2.clone()),
            origin_vertipad_id: vertipad_1.clone(),
            target_vertipad_id: vertipad_2.clone(),
            vehicle_id,
            path: Some(GeoLineString { points: vec![] }),
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
        assert_eq!(
            itinerary[0].origin_vertiport_id.clone().unwrap(),
            vertiport_1
        );
        assert_eq!(
            itinerary[0].target_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[0].target_vertipad_id.clone(), vertipad_1);

        assert_eq!(
            itinerary[1].origin_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[1].origin_vertipad_id.clone(), vertipad_1);
        assert_eq!(
            itinerary[1].target_vertiport_id.clone().unwrap(),
            vertiport_2
        );
        assert_eq!(itinerary[1].target_vertipad_id.clone(), vertipad_2);

        assert_eq!(
            itinerary[2].origin_vertiport_id.clone().unwrap(),
            vertiport_2
        );
        assert_eq!(itinerary[2].origin_vertipad_id.clone(), vertipad_2);
        assert_eq!(
            itinerary[2].target_vertiport_id.clone().unwrap(),
            vertiport_1
        );

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
        let time_end = Utc::now() + Duration::seconds(1000);
        let vertiport_1 = Uuid::new_v4().to_string();
        let vertiport_3 = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();
        let vehicle_id = Uuid::new_v4().to_string();
        let required_loading_time = Duration::seconds(30);
        let required_unloading_time = Duration::seconds(30);

        let aircraft_availability = Availability {
            vertiport_id: vertiport_1.clone(),
            vertipad_id: vertipad_1.clone(),
            timeslot: Timeslot {
                time_start: time_start - Duration::seconds(1000),
                time_end: time_end + Duration::seconds(1000),
            },
        };

        let distance_meters = 50.0;
        let flight_duration = estimate_flight_time_seconds(&distance_meters);
        let flight_window = Timeslot {
            time_end,
            time_start,
        };

        let flight_plan = flight_plan::Data {
            origin_vertiport_id: Some(vertiport_3.clone()),
            target_vertiport_id: Some(vertiport_1.clone()),
            origin_vertipad_id: vertipad_1.clone(),
            target_vertipad_id: vertipad_2.clone(),
            vehicle_id,
            path: Some(GeoLineString { points: vec![] }),
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
        assert_eq!(
            itinerary[0].origin_vertiport_id.clone().unwrap(),
            vertiport_1
        );
        assert_eq!(
            itinerary[0].target_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[0].target_vertipad_id.clone(), vertipad_1);

        assert_eq!(
            itinerary[1].origin_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[1].origin_vertipad_id.clone(), vertipad_1);
        assert_eq!(
            itinerary[1].target_vertiport_id.clone().unwrap(),
            vertiport_1
        );
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
        let time_end = Utc::now() + Duration::seconds(1000);
        let vertiport_1 = Uuid::new_v4().to_string();
        let vertiport_3 = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();
        let vehicle_id = Uuid::new_v4().to_string();
        let required_loading_time = Duration::seconds(30);
        let required_unloading_time = Duration::seconds(30);

        let aircraft_availability = Availability {
            vertiport_id: vertiport_1.clone(),
            vertipad_id: vertipad_1.clone(),
            timeslot: Timeslot {
                time_start: time_start - Duration::seconds(1000),
                time_end,
            },
        };

        let distance_meters = 50.0;
        let flight_duration = estimate_flight_time_seconds(&distance_meters);
        let flight_window = Timeslot {
            time_start,
            time_end,
        };

        let flight_plan = flight_plan::Data {
            origin_vertiport_id: Some(vertiport_1.clone()),
            target_vertiport_id: Some(vertiport_3.clone()),
            origin_vertipad_id: vertipad_1.clone(),
            target_vertipad_id: vertipad_2.clone(),
            vehicle_id,
            path: Some(GeoLineString { points: vec![] }),
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
        assert_eq!(
            itinerary[0].origin_vertiport_id.clone().unwrap(),
            vertiport_1
        );
        assert_eq!(itinerary[0].origin_vertipad_id.clone(), vertipad_1);
        assert_eq!(
            itinerary[0].target_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[0].target_vertipad_id.clone(), vertipad_2);

        assert_eq!(
            itinerary[1].origin_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[1].origin_vertipad_id.clone(), vertipad_2);
        assert_eq!(
            itinerary[1].target_vertiport_id.clone().unwrap(),
            vertiport_1
        );

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
        let time_end = Utc::now() + Duration::hours(1);
        let vertiport_1 = Uuid::new_v4().to_string();
        let vertiport_2 = Uuid::new_v4().to_string();
        let vertiport_3 = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();
        let vehicle_id = Uuid::new_v4().to_string();
        let required_loading_time = Duration::seconds(30);
        let required_unloading_time = Duration::seconds(30);

        //       |    flight window  |
        //  |     takeoff and land time window     |
        //

        let aircraft_availability = Availability {
            vertiport_id: vertiport_1.clone(),
            vertipad_id: vertipad_1.clone(),
            timeslot: Timeslot {
                time_start: time_start + Duration::minutes(10),
                time_end: time_end - Duration::minutes(20),
            },
        };

        let distance_meters = 50.0;
        let flight_duration = estimate_flight_time_seconds(&distance_meters);
        let flight_window = Timeslot {
            time_end,
            time_start,
        };

        let flight_plan = flight_plan::Data {
            origin_vertiport_id: Some(vertiport_3.clone()),
            target_vertiport_id: Some(vertiport_2.clone()),
            origin_vertipad_id: vertipad_1.clone(),
            target_vertipad_id: vertipad_2.clone(),
            vehicle_id,
            path: Some(GeoLineString { points: vec![] }),
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
        assert_eq!(
            itinerary[0].origin_vertiport_id.clone().unwrap(),
            vertiport_1
        );
        assert_eq!(
            itinerary[0].target_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[0].target_vertipad_id.clone(), vertipad_1);

        assert_eq!(
            itinerary[1].origin_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[1].origin_vertipad_id.clone(), vertipad_1);
        assert_eq!(
            itinerary[1].target_vertiport_id.clone().unwrap(),
            vertiport_2
        );
        assert_eq!(itinerary[1].target_vertipad_id.clone(), vertipad_2);

        assert_eq!(
            itinerary[2].origin_vertiport_id.clone().unwrap(),
            vertiport_2
        );
        assert_eq!(itinerary[2].origin_vertipad_id.clone(), vertipad_2);
        assert_eq!(
            itinerary[2].target_vertiport_id.clone().unwrap(),
            vertiport_1
        );

        // First itinerary for aircraft leaves at earliest aircraft convenience
        assert_eq!(
            itinerary[0].origin_timeslot_start.clone().unwrap(),
            aircraft_availability.timeslot.time_start.into()
        );
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn test_get_itinerary_valid_incompatible_flight_window() {
        let clients = get_clients().await;
        let time_start = Utc::now();
        let time_end = Utc::now() + Duration::hours(1);
        let vertiport_1 = Uuid::new_v4().to_string();
        let vertiport_2 = Uuid::new_v4().to_string();
        let vertiport_3 = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();
        let vehicle_id = Uuid::new_v4().to_string();
        let required_loading_time = Duration::seconds(30);
        let required_unloading_time = Duration::seconds(30);

        //                                       |    flight window    |
        //  |     takeoff and land time window     |
        //

        let aircraft_availability = Availability {
            vertiport_id: vertiport_1.clone(),
            vertipad_id: vertipad_1.clone(),
            timeslot: Timeslot {
                time_start: time_end - Duration::seconds(30),
                time_end: time_end + Duration::minutes(20),
            },
        };

        let distance_meters = 1000.0; // too far to fly
        let flight_duration = estimate_flight_time_seconds(&distance_meters);
        let flight_window = Timeslot {
            time_end,
            time_start,
        };

        let flight_plan = flight_plan::Data {
            origin_vertiport_id: Some(vertiport_3.clone()),
            target_vertiport_id: Some(vertiport_2.clone()),
            origin_vertipad_id: vertipad_1.clone(),
            target_vertipad_id: vertipad_2.clone(),
            vehicle_id,
            path: Some(GeoLineString { points: vec![] }),
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
        let time_end = Utc::now() + Duration::seconds(1000);
        let vertiport_1 = Uuid::new_v4().to_string();
        let vertiport_2 = Uuid::new_v4().to_string();
        let vertiport_3 = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_3 = Uuid::new_v4().to_string();
        let vehicle_1 = Uuid::new_v4().to_string();
        let vehicle_2 = Uuid::new_v4().to_string();
        let required_loading_time = Duration::seconds(30);
        let required_unloading_time = Duration::seconds(30);

        let availabilities = HashMap::from([
            (
                vehicle_1.clone(),
                vec![Availability {
                    vertiport_id: vertiport_1.clone(),
                    vertipad_id: vertipad_1.clone(),
                    timeslot: Timeslot {
                        time_start: time_start - Duration::hours(1),
                        time_end: time_end + Duration::hours(1),
                    },
                }],
            ),
            (
                vehicle_2.clone(),
                vec![Availability {
                    vertiport_id: vertiport_3.clone(),
                    vertipad_id: vertipad_3.clone(),
                    timeslot: Timeslot {
                        time_start: time_end + Duration::hours(1),
                        time_end: time_end + Duration::hours(2),
                    },
                }],
            ),
        ]);

        let distance_meters = 50.0;
        let flight_duration = estimate_flight_time_seconds(&distance_meters);
        let timeslot_pairs = vec![
            TimeslotPair {
                origin_port_id: vertiport_1.clone(),
                origin_pad_id: vertipad_1.clone(),
                origin_timeslot: Timeslot {
                    time_start: time_start.clone(),
                    time_end: time_end.clone(),
                },
                target_port_id: vertiport_2.clone(),
                target_pad_id: vertiport_2.clone(),
                target_timeslot: Timeslot {
                    time_start: time_start + flight_duration,
                    time_end: time_end + flight_duration,
                },
                path: GeoLineString { points: vec![] },
                distance_meters,
            },
            TimeslotPair {
                origin_port_id: vertiport_1.clone(),
                origin_pad_id: vertipad_1.clone(),
                origin_timeslot: Timeslot {
                    time_start: time_end + Duration::hours(1),
                    time_end: time_end + Duration::hours(2),
                },
                target_port_id: vertiport_2.clone(),
                target_pad_id: vertiport_2.clone(),
                target_timeslot: Timeslot {
                    time_start: time_end + Duration::hours(1) + flight_duration,
                    time_end: time_end + Duration::hours(2) + flight_duration,
                },
                path: GeoLineString { points: vec![] },
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
}
