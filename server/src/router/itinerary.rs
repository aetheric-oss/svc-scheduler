//! Build an itinerary given aircraft availability and the flight window

use super::schedule::*;
use super::vehicle::*;
use super::vertiport::TimeslotPair;
use super::{best_path, BestPathError, BestPathRequest};
use crate::grpc::client::GrpcClients;
use svc_gis_client_grpc::prelude::gis::*;
use svc_storage_client_grpc::prelude::*;

use chrono::{DateTime, Duration, Utc};
use std::cmp::max;
use std::collections::HashMap;
use std::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Debug, Clone, PartialEq)]
pub enum ItineraryError {
    ClientError,
    InvalidData,
    NoPathFound,
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

/// Given timeslot pairs for departure and arrival vertiport and the
///  availabilities of the aircraft, get possible itineraries for each
///  aircraft.
/// Returns a maximum of 1 itinerary per aircraft.
pub async fn get_itineraries(
    required_loading_time: &Duration,
    required_unloading_time: &Duration,
    timeslot_pairs: &[TimeslotPair],
    aircraft_gaps: &HashMap<String, Vec<Availability>>,
    clients: &GrpcClients,
) -> Result<Vec<Vec<flight_plan::Data>>, ItineraryError> {
    let mut itineraries: Vec<Vec<flight_plan::Data>> = vec![];

    // For each available aircraft, see if it can do the flight
    for (aircraft_id, aircraft_availability) in aircraft_gaps {
        // Try different timeslots for the aircraft
        for pair in timeslot_pairs {
            // TODO(R4): Include vehicle model to improve estimate
            let flight_duration = estimate_flight_time_seconds(&pair.distance_meters);

            let flight_window = Timeslot {
                time_start: pair.depart_timeslot.time_start,
                time_end: pair.arrival_timeslot.time_end,
            };

            let flight_plan = svc_storage_client_grpc::prelude::flight_plan::Data {
                departure_vertiport_id: Some(pair.depart_port_id.clone()),
                destination_vertiport_id: Some(pair.arrival_port_id.clone()),
                departure_vertipad_id: pair.depart_pad_id.clone(),
                destination_vertipad_id: pair.arrival_pad_id.clone(),
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
                        "(get_vehicle_availability) Could not determine path; client error."
                    );
                    return Err(ItineraryError::ClientError);
                }
                _ => {
                    continue;
                }
            }
        }
    }

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
                router_error!("(get_vehicle_availability) Could not determine path; client error.");
                return Err(ItineraryError::ClientError);
            }
            _ => {
                continue;
            }
        }
    }

    Err(ItineraryError::ScheduleConflict)
}

/// Determines if the aircraft is available for the requested flight,
///  given that it may require multiple deadhead trips.
async fn get_itinerary(
    flight_plan: flight_plan::Data,
    availability: &Availability,
    flight_duration: &Duration,
    _required_loading_time: &Duration,
    _required_unloading_time: &Duration,
    flight_window: &Timeslot,
    clients: &GrpcClients,
) -> Result<Vec<flight_plan::Data>, ItineraryError> {
    // Must be some overlap between the flight window and the available timeslot
    let Ok(overlap) = availability.timeslot.overlap(flight_window) else {
        router_debug!(
            "(is_aircraft_available) No overlap between flight window and available timeslot."
        );

        return Err(ItineraryError::ScheduleConflict);
    };

    let Some(ref departure_vertiport_id) = flight_plan.departure_vertiport_id else {
        router_error!(
            "(get_vehicle_itinerary) Flight plan doesn't have departure_vertiport_id.",
        );

        return Err(ItineraryError::InvalidData);
    };

    let Some(ref arrival_vertiport_id) = flight_plan.destination_vertiport_id else {
        router_error!(
            "(get_vehicle_itinerary) Flight plan doesn't have destination_vertiport_id.",
        );

        return Err(ItineraryError::InvalidData);
    };

    let vehicle_id = flight_plan.vehicle_id.clone();

    //
    // Create the flight plan for the deadhead flight to the requested departure vertiport
    //
    let mut flight_plans = vec![];
    if *departure_vertiport_id != availability.vertiport_id {
        // See what the path and cost would be for a flight between the starting
        // available timeslot and the ending flight time
        let best_path_request = BestPathRequest {
            start_type: NodeType::Vertiport as i32,
            node_start_id: availability.vertiport_id.clone(),
            node_uuid_end: departure_vertiport_id.clone(),
            time_start: Some(availability.timeslot.time_start.into()),
            time_end: Some(overlap.time_end.into()),
        };

        let (deadhead_path, pre_deadhead_distance_meters) =
            match best_path(&best_path_request, clients).await {
                Ok((deadhead_path, d)) => (deadhead_path, d as f32),
                Err(BestPathError::NoPathFound) => {
                    // no path found, perhaps temporary no-fly zone
                    //  is blocking journeys from this depart timeslot
                    // Break out and try the next depart timeslot
                    router_debug!(
                        "(get_vehicle_availability) No path found from vertiport {}
                to vertiport {} (from {} to {}).",
                        availability.vertiport_id,
                        departure_vertiport_id,
                        availability.timeslot.time_start,
                        availability.timeslot.time_end
                    );

                    return Err(ItineraryError::NoPathFound);
                }
                Err(BestPathError::ClientError) => {
                    // exit immediately if svc-gis is down, don't allow new flights
                    router_error!("(get_vehicle_availability) Could not determine path.");
                    return Err(ItineraryError::ClientError);
                }
            };

        let pre_deadhead_duration = estimate_flight_time_seconds(&pre_deadhead_distance_meters);

        // leave at earliest possible time
        let scheduled_departure = max(
            availability.timeslot.time_start,
            flight_window.time_start - pre_deadhead_duration,
        );
        let scheduled_arrival = scheduled_departure + pre_deadhead_duration;
        if scheduled_arrival > availability.timeslot.time_end {
            // This flight plan would end after the available timeslot
            //  Break out and try the next available timeslot
            router_debug!(
                "(get_vehicle_availability) Flight plan would end after available timeslot."
            );

            return Err(ItineraryError::ScheduleConflict);
        }

        // TODO(R4): Get last vertipad for departure_vertipad_id
        //  less important than knowing where you're going to land
        flight_plans.push(flight_plan::Data {
            scheduled_departure: Some(scheduled_departure.into()),
            scheduled_arrival: Some(scheduled_arrival.into()),
            // go from current location, known from availability, to the departure vertiport for the requested flight
            departure_vertiport_id: Some(availability.vertiport_id.clone()),
            destination_vertiport_id: Some(departure_vertiport_id.clone()),
            destination_vertipad_id: flight_plan.departure_vertipad_id.clone(),
            vehicle_id: vehicle_id.clone(),
            path: Some(deadhead_path),
            ..Default::default()
        });
    }

    //
    // Create the flight plan for the requested flight
    //
    {
        let scheduled_departure: DateTime<Utc> = match flight_plans.last() {
            Some(last) => match &last.scheduled_arrival {
                Some(s) => s.clone().into(),
                None => {
                    router_error!(
                        "(get_vehicle_availability) Last flight plan has no scheduled arrival."
                    );

                    return Err(ItineraryError::InvalidData);
                }
            },
            // leave at earliest possible time
            None => max(flight_window.time_start, availability.timeslot.time_start),
        };

        let scheduled_arrival = scheduled_departure + *flight_duration;
        if scheduled_arrival > availability.timeslot.time_end {
            // This flight plan would end after the available timeslot
            //  Break out and try the next available timeslot
            router_debug!(
                "(get_vehicle_availability) Flight plan would end after available timeslot."
            );

            return Err(ItineraryError::ScheduleConflict);
        }

        if scheduled_arrival > flight_window.time_end {
            // This flight plan would end after the flight window
            //  Break out and try the next available timeslot
            router_debug!("(get_vehicle_availability) Flight plan would end after flight window.");

            return Err(ItineraryError::ScheduleConflict);
        }

        // Flight requested by user
        let mut flight_plan = flight_plan.clone();
        flight_plan.scheduled_departure = Some(scheduled_departure.into());
        flight_plan.scheduled_arrival = Some(scheduled_arrival.into());
        flight_plans.push(flight_plan);
    }

    //
    // Create the post deadhead flight to take the aircraft away from the pad
    //  when flight is completed
    //
    if *arrival_vertiport_id != availability.vertiport_id {
        // TODO(R4) - Get nearest open rest stop/hangar, direct to it
        //  right now it boomerangs back to its original last_vertiport_id
        let Some(last) = flight_plans.last() else {
            router_error!(
                "(get_vehicle_availability) No flight plans found for vehicle {}.",
                vehicle_id
            );

            return Err(ItineraryError::InvalidData);
        };

        let Some(last_arrival) = &last.scheduled_arrival else {
            router_error!(
                "(get_vehicle_availability) Last flight plan has no scheduled arrival."
            );

            return Err(ItineraryError::InvalidData);
        };

        // See what the path would cost from the flight plan's destination port
        //  to the next flight plan's departure port
        let best_path_request = BestPathRequest {
            start_type: NodeType::Vertiport as i32,
            node_start_id: arrival_vertiport_id.clone(),
            node_uuid_end: availability.vertiport_id.clone(),
            time_start: Some(last_arrival.clone()),
            time_end: Some(availability.timeslot.time_end.into()),
        };

        let (deadhead_path, post_deadhead_distance_meters) =
            match best_path(&best_path_request, clients).await {
                Ok((deadhead_path, d)) => (deadhead_path, d as f32),
                Err(BestPathError::NoPathFound) => {
                    // no path found, perhaps temporary no-fly zone
                    //  is blocking journeys from this depart timeslot
                    // Break out and try the next depart timeslot
                    router_debug!(
                        "(get_vehicle_availability) No path found from vertiport {}
                to vertiport {} (from {} to {}).",
                        arrival_vertiport_id,
                        availability.vertiport_id,
                        availability.timeslot.time_start,
                        availability.timeslot.time_end
                    );

                    return Err(ItineraryError::NoPathFound);
                }
                Err(BestPathError::ClientError) => {
                    // exit immediately if svc-gis is down, don't allow new flights
                    router_error!("(get_vehicle_availability) Could not determine path.");
                    return Err(ItineraryError::ClientError);
                }
            };

        let post_deadhead_duration = estimate_flight_time_seconds(&post_deadhead_distance_meters);

        let scheduled_departure: DateTime<Utc> = last_arrival.clone().into();
        let scheduled_arrival = scheduled_departure + post_deadhead_duration;
        if scheduled_arrival > availability.timeslot.time_end {
            // This flight plan would end after the available timeslot
            //  Break out and try the next available timeslot
            router_debug!(
                "(get_vehicle_availability) Flight plan would end after available timeslot."
            );

            return Err(ItineraryError::ScheduleConflict);
        }

        // TODO(R4): Get open vertipad for deadhead to rest stop/hangar

        flight_plans.push(flight_plan::Data {
            scheduled_departure: Some(scheduled_departure.into()),
            scheduled_arrival: Some(scheduled_arrival.into()),
            // go from current location, known from availability, to the departure vertiport for the requested flight
            departure_vertiport_id: last.destination_vertiport_id.clone(),
            departure_vertipad_id: last.destination_vertipad_id.clone(),
            destination_vertiport_id: Some(availability.vertiport_id.clone()),
            vehicle_id: vehicle_id.clone(),
            path: Some(deadhead_path),
            ..Default::default()
        });
    }

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
        let time_end = Utc::now() + Duration::seconds(100);
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
            timeslot: Timeslot {
                time_start: time_start - Duration::seconds(100),
                time_end: time_end + Duration::seconds(100),
            },
        };

        let distance_meters = 50.0;
        let flight_duration = estimate_flight_time_seconds(&distance_meters);
        let flight_window = Timeslot {
            time_end,
            time_start,
        };

        let flight_plan = flight_plan::Data {
            departure_vertiport_id: Some(vertiport_3.clone()),
            destination_vertiport_id: Some(vertiport_2.clone()),
            departure_vertipad_id: vertipad_1.clone(),
            destination_vertipad_id: vertipad_2.clone(),
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
            itinerary[0].departure_vertiport_id.clone().unwrap(),
            vertiport_1
        );
        assert_eq!(
            itinerary[0].destination_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[0].destination_vertipad_id.clone(), vertipad_1);

        assert_eq!(
            itinerary[1].departure_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[1].departure_vertipad_id.clone(), vertipad_1);
        assert_eq!(
            itinerary[1].destination_vertiport_id.clone().unwrap(),
            vertiport_2
        );
        assert_eq!(itinerary[1].destination_vertipad_id.clone(), vertipad_2);

        assert_eq!(
            itinerary[2].departure_vertiport_id.clone().unwrap(),
            vertiport_2
        );
        assert_eq!(itinerary[2].departure_vertipad_id.clone(), vertipad_2);
        assert_eq!(
            itinerary[2].destination_vertiport_id.clone().unwrap(),
            vertiport_1
        );

        // Land at earliest possible time
        assert_eq!(
            itinerary[0].scheduled_arrival.clone().unwrap(),
            time_start.into()
        );
        assert_eq!(
            itinerary[1].scheduled_departure.clone().unwrap(),
            time_start.into()
        );
        assert_eq!(
            itinerary[1].scheduled_arrival.clone().unwrap(),
            (time_start + flight_duration).into()
        );
        assert_eq!(
            itinerary[2].scheduled_departure.clone().unwrap(),
            (time_start + flight_duration).into()
        );
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn test_get_itinerary_valid_pre_deadhead() {
        let clients = get_clients().await;
        let time_start = Utc::now();
        let time_end = Utc::now() + Duration::seconds(100);
        let vertiport_1 = Uuid::new_v4().to_string();
        let vertiport_3 = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();
        let vehicle_id = Uuid::new_v4().to_string();
        let required_loading_time = Duration::seconds(30);
        let required_unloading_time = Duration::seconds(30);

        let aircraft_availability = Availability {
            vertiport_id: vertiport_1.clone(),
            timeslot: Timeslot {
                time_start: time_start - Duration::seconds(100),
                time_end: time_end + Duration::seconds(100),
            },
        };

        let distance_meters = 50.0;
        let flight_duration = estimate_flight_time_seconds(&distance_meters);
        let flight_window = Timeslot {
            time_end,
            time_start,
        };

        let flight_plan = flight_plan::Data {
            departure_vertiport_id: Some(vertiport_3.clone()),
            destination_vertiport_id: Some(vertiport_1.clone()),
            departure_vertipad_id: vertipad_1.clone(),
            destination_vertipad_id: vertipad_2.clone(),
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
            itinerary[0].departure_vertiport_id.clone().unwrap(),
            vertiport_1
        );
        assert_eq!(
            itinerary[0].destination_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[0].destination_vertipad_id.clone(), vertipad_1);

        assert_eq!(
            itinerary[1].departure_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[1].departure_vertipad_id.clone(), vertipad_1);
        assert_eq!(
            itinerary[1].destination_vertiport_id.clone().unwrap(),
            vertiport_1
        );
        assert_eq!(itinerary[1].destination_vertipad_id.clone(), vertipad_2);

        // Land at earliest possible time
        assert_eq!(
            itinerary[0].scheduled_arrival.clone().unwrap(),
            time_start.into()
        );
        assert_eq!(
            itinerary[1].scheduled_departure.clone().unwrap(),
            time_start.into()
        );
        assert_eq!(
            itinerary[1].scheduled_arrival.clone().unwrap(),
            (time_start + flight_duration).into()
        );
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn test_get_itinerary_valid_post_deadhead() {
        let clients = get_clients().await;
        let time_start = Utc::now();
        let time_end = Utc::now() + Duration::seconds(100);
        let vertiport_1 = Uuid::new_v4().to_string();
        let vertiport_3 = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let vertipad_2 = Uuid::new_v4().to_string();
        let vehicle_id = Uuid::new_v4().to_string();
        let required_loading_time = Duration::seconds(30);
        let required_unloading_time = Duration::seconds(30);

        let aircraft_availability = Availability {
            vertiport_id: vertiport_1.clone(),
            timeslot: Timeslot {
                time_start: time_start - Duration::seconds(100),
                time_end: time_end + Duration::seconds(100),
            },
        };

        let distance_meters = 50.0;
        let flight_duration = estimate_flight_time_seconds(&distance_meters);
        let flight_window = Timeslot {
            time_end,
            time_start,
        };

        let flight_plan = flight_plan::Data {
            departure_vertiport_id: Some(vertiport_1.clone()),
            destination_vertiport_id: Some(vertiport_3.clone()),
            departure_vertipad_id: vertipad_1.clone(),
            destination_vertipad_id: vertipad_2.clone(),
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
            itinerary[0].departure_vertiport_id.clone().unwrap(),
            vertiport_1
        );
        assert_eq!(itinerary[0].departure_vertipad_id.clone(), vertipad_1);
        assert_eq!(
            itinerary[0].destination_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[0].destination_vertipad_id.clone(), vertipad_2);

        assert_eq!(
            itinerary[1].departure_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[1].departure_vertipad_id.clone(), vertipad_2);
        assert_eq!(
            itinerary[1].destination_vertiport_id.clone().unwrap(),
            vertiport_1
        );

        // Land at earliest possible time
        assert_eq!(
            itinerary[0].scheduled_departure.clone().unwrap(),
            time_start.into()
        );
        assert_eq!(
            itinerary[0].scheduled_arrival.clone().unwrap(),
            (time_start + flight_duration).into()
        );
        assert_eq!(
            itinerary[1].scheduled_departure.clone().unwrap(),
            (time_start + flight_duration).into()
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
            departure_vertiport_id: Some(vertiport_3.clone()),
            destination_vertiport_id: Some(vertiport_2.clone()),
            departure_vertipad_id: vertipad_1.clone(),
            destination_vertipad_id: vertipad_2.clone(),
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
            itinerary[0].departure_vertiport_id.clone().unwrap(),
            vertiport_1
        );
        assert_eq!(
            itinerary[0].destination_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[0].destination_vertipad_id.clone(), vertipad_1);

        assert_eq!(
            itinerary[1].departure_vertiport_id.clone().unwrap(),
            vertiport_3
        );
        assert_eq!(itinerary[1].departure_vertipad_id.clone(), vertipad_1);
        assert_eq!(
            itinerary[1].destination_vertiport_id.clone().unwrap(),
            vertiport_2
        );
        assert_eq!(itinerary[1].destination_vertipad_id.clone(), vertipad_2);

        assert_eq!(
            itinerary[2].departure_vertiport_id.clone().unwrap(),
            vertiport_2
        );
        assert_eq!(itinerary[2].departure_vertipad_id.clone(), vertipad_2);
        assert_eq!(
            itinerary[2].destination_vertiport_id.clone().unwrap(),
            vertiport_1
        );

        // First itinerary for aircraft leaves at earliest aircraft convenience
        assert_eq!(
            itinerary[0].scheduled_departure.clone().unwrap(),
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
            departure_vertiport_id: Some(vertiport_3.clone()),
            destination_vertiport_id: Some(vertiport_2.clone()),
            departure_vertipad_id: vertipad_1.clone(),
            destination_vertipad_id: vertipad_2.clone(),
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
    async fn test_get_itineraries() {
        let clients = get_clients().await;
        let time_start = Utc::now();
        let time_end = Utc::now() + Duration::seconds(100);
        let vertiport_1 = Uuid::new_v4().to_string();
        let vertiport_2 = Uuid::new_v4().to_string();
        let vertiport_3 = Uuid::new_v4().to_string();
        let vertipad_1 = Uuid::new_v4().to_string();
        let _vertipad_2 = Uuid::new_v4().to_string();
        let vehicle_1 = Uuid::new_v4().to_string();
        let vehicle_2 = Uuid::new_v4().to_string();
        let required_loading_time = Duration::seconds(30);
        let required_unloading_time = Duration::seconds(30);

        let availabilities = HashMap::from([
            (
                vehicle_1.clone(),
                vec![Availability {
                    vertiport_id: vertiport_1.clone(),
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
                depart_port_id: vertiport_1.clone(),
                depart_pad_id: vertipad_1.clone(),
                depart_timeslot: Timeslot {
                    time_start: time_start.clone(),
                    time_end: time_end.clone(),
                },
                arrival_port_id: vertiport_2.clone(),
                arrival_pad_id: vertiport_2.clone(),
                arrival_timeslot: Timeslot {
                    time_start: time_start + flight_duration,
                    time_end: time_end + flight_duration,
                },
                path: GeoLineString { points: vec![] },
                distance_meters,
            },
            TimeslotPair {
                depart_port_id: vertiport_1.clone(),
                depart_pad_id: vertipad_1.clone(),
                depart_timeslot: Timeslot {
                    time_start: time_end + Duration::hours(1),
                    time_end: time_end + Duration::hours(2),
                },
                arrival_port_id: vertiport_2.clone(),
                arrival_pad_id: vertiport_2.clone(),
                arrival_timeslot: Timeslot {
                    time_start: time_end + Duration::hours(1) + flight_duration,
                    time_end: time_end + Duration::hours(2) + flight_duration,
                },
                path: GeoLineString { points: vec![] },
                distance_meters,
            },
        ];

        let itineraries = get_itineraries(
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
