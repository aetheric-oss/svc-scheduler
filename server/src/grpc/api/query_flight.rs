//! This module contains the gRPC query_flight endpoint implementation.

use chrono::{DateTime, Duration, Utc};
use std::fmt::{Display, Formatter, Result as FmtResult};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::grpc::client::get_clients;
use crate::grpc::server::grpc_server::{Itinerary, QueryFlightRequest, QueryFlightResponse};

use crate::router::flight_plan::*;
use crate::router::itinerary::get_itineraries;
use crate::router::schedule::*;
use crate::router::vehicle::*;
use crate::router::vertiport::*;
use svc_storage_client_grpc::prelude::flight_plan::Object as FlightPlanObject;

/// Time to block vertiport for cargo loading and takeoff
pub const LOADING_AND_TAKEOFF_TIME_SECONDS: i64 = 600;
/// Time to block vertiport for cargo unloading and landing
pub const LANDING_AND_UNLOADING_TIME_SECONDS: i64 = 600;
/// Maximum time between departure and arrival times for flight queries
pub const MAX_FLIGHT_QUERY_WINDOW_MINUTES: i64 = 360; // +/- 3 hours (6 total)

/// Sanitized version of the gRPC query
struct FlightQuery {
    departure_vertiport_id: String,
    arrival_vertiport_id: String,
    earliest_departure_time: DateTime<Utc>,
    latest_arrival_time: DateTime<Utc>,
    required_loading_time: Duration,
    required_unloading_time: Duration,
}

/// Error type for FlightQuery
#[derive(Debug, Clone, Copy)]
enum FlightQueryError {
    InvalidVertiportId,
    InvalidTime,
    TimeRangeTooLarge,
}

impl Display for FlightQueryError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            FlightQueryError::InvalidVertiportId => write!(f, "Invalid vertiport ID"),
            FlightQueryError::InvalidTime => write!(f, "Invalid time"),
            FlightQueryError::TimeRangeTooLarge => write!(f, "Time range too large"),
        }
    }
}

impl TryFrom<QueryFlightRequest> for FlightQuery {
    type Error = FlightQueryError;

    fn try_from(request: QueryFlightRequest) -> Result<Self, Self::Error> {
        const ERROR_PREFIX: &str = "(TryFrom<QueryFlightRequest> FlightQuery)";

        let departure_vertiport_id = match Uuid::parse_str(&request.vertiport_depart_id) {
            Ok(id) => id.to_string(),
            _ => {
                grpc_error!(
                    "(FlightQuery::TryFrom) {} Invalid departure vertiport ID: {}",
                    ERROR_PREFIX,
                    request.vertiport_depart_id
                );
                return Err(FlightQueryError::InvalidVertiportId);
            }
        };

        let arrival_vertiport_id = match Uuid::parse_str(&request.vertiport_arrive_id) {
            Ok(id) => id.to_string(),
            _ => {
                grpc_error!(
                    "(FlightQuery::TryFrom) {} Invalid departure vertiport ID: {}",
                    ERROR_PREFIX,
                    request.vertiport_arrive_id
                );
                return Err(FlightQueryError::InvalidVertiportId);
            }
        };

        let Some(latest_arrival_time) = request.latest_arrival_time.clone() else {
            grpc_warn!("(FlightQuery::TryFrom) {} latest arrival time not provided.", ERROR_PREFIX);
            return Err(FlightQueryError::InvalidTime);
        };

        let Some(earliest_departure_time) = request.earliest_departure_time else {
            grpc_warn!("(FlightQuery::TryFrom) {} earliest departure time not provided.", ERROR_PREFIX);
            return Err(FlightQueryError::InvalidTime);
        };

        let latest_arrival_time: DateTime<Utc> = latest_arrival_time.into();
        let earliest_departure_time: DateTime<Utc> = earliest_departure_time.into();

        if earliest_departure_time > latest_arrival_time {
            grpc_warn!(
                "(FlightQuery::TryFrom) {} earliest departure time is after latest arrival time.",
                ERROR_PREFIX
            );
            return Err(FlightQueryError::InvalidTime);
        }

        // Prevent attacks where a user requests a wide flight window, resulting in a large number of
        //  calls to svc-gis for routing
        if latest_arrival_time - earliest_departure_time
            > Duration::minutes(MAX_FLIGHT_QUERY_WINDOW_MINUTES)
        {
            grpc_warn!(
                "(FlightQuery::TryFrom) {} time range too large.",
                ERROR_PREFIX
            );
            return Err(FlightQueryError::TimeRangeTooLarge);
        }

        if latest_arrival_time < Utc::now() {
            grpc_warn!(
                "(FlightQuery::TryFrom) {} latest arrival time is in the past.",
                ERROR_PREFIX
            );
            return Err(FlightQueryError::InvalidTime);
        }

        Ok(FlightQuery {
            departure_vertiport_id,
            arrival_vertiport_id,
            latest_arrival_time,
            earliest_departure_time,
            // TODO(R4): Get needed loading/unloading times from request
            required_loading_time: Duration::seconds(LOADING_AND_TAKEOFF_TIME_SECONDS),
            required_unloading_time: Duration::seconds(LANDING_AND_UNLOADING_TIME_SECONDS),
        })
    }
}

/// Finds the first possible flight for customer location, flight type and requested time.
/// TODO(R5): Return a stream of messages for live updates on query progress
pub async fn query_flight(
    request: Request<QueryFlightRequest>,
) -> Result<Response<QueryFlightResponse>, Status> {
    let request = request.into_inner();
    let request = match FlightQuery::try_from(request) {
        Ok(request) => request,
        Err(e) => {
            let error_str = "Invalid flight query request";
            grpc_error!("(query_flight) {error_str}: {e}");
            return Err(Status::invalid_argument(error_str));
        }
    };

    let clients = get_clients().await;

    // TODO(R4): Don't get flight plans until we have a vertipad timeslot match - may not need to
    //   get existing flight plans with the vertipad_timeslot call planned for R4
    // Get all flight plans from this time to latest departure time (including partially fitting flight plans)
    // - this assumes that all landed flights have updated vehicle.last_vertiport_id (otherwise we would need to look in to the past)
    let existing_flight_plans =
        match get_sorted_flight_plans(&request.latest_arrival_time, clients).await {
            Ok(plans) => plans,
            Err(e) => {
                let error_str = "Could not get existing flight plans.";
                grpc_error!("(query_flight) {} {}", error_str, e);
                return Err(Status::internal(error_str));
            }
        };

    // Add draft flight plans to the "existing" flight plans
    // let Ok(draft_flight_plans) = super::unconfirmed_flight_plans().lock() else {
    //     let error_str = "Could not get draft flight plans.";
    //     grpc_error!("(query_flight) {} {}", error_str, e);
    //     return Err(Status::internal(error_str));
    // };

    // draft_flight_plans.iter().for_each(|(_, fp)| {
    //     // only push flight plans for aircraft that we have in our list
    //     // don't want to schedule new flights for removed aircraft
    //     if let Some(schedule) = aircraft.get_mut(&fp.vehicle_id) {
    //         schedule.push(fp.clone());
    //     } else {
    //         grpc_warn!(
    //             "(query_flight) Flight plan for unknown aircraft: {}",
    //             fp.vehicle_id
    //         );
    //     }
    // });

    //
    // TODO(R4): Determine if there's an open space for cargo on an existing flight plan
    //

    grpc_debug!(
        "(query_flight) found existing flight plans: {:?}",
        existing_flight_plans
    );

    let timeslot = Timeslot {
        time_start: request.earliest_departure_time,
        time_end: request.latest_arrival_time,
    };

    //
    // Get available timeslots for departure vertiport that are large enough to
    //  fit the required loading and takeoff time.
    //
    let Ok(timeslot_pairs) = get_timeslot_pairs(
        &request.departure_vertiport_id,
        &request.arrival_vertiport_id,
        &request.required_loading_time,
        &request.required_unloading_time,
        &timeslot,
        &existing_flight_plans,
        clients,
    ).await else {
        let error_str = "Could not find a timeslot pairing.";
        grpc_error!("(query_flight) {}", error_str);
        return Err(Status::internal(error_str));
    };

    if timeslot_pairs.is_empty() {
        let info_str = "No routes available for the given time.";
        grpc_info!("(query_flight) {info_str}");
        return Err(Status::not_found(info_str));
    }

    //
    // Get all aircraft availabilities
    //
    let Ok(aircraft_gaps) = get_aircraft_gaps(
        &existing_flight_plans,
        clients
    ).await else {
        let error_str = "Could not get aircraft availabilities.";
        grpc_error!("(query_flight) {}", error_str);
        return Err(Status::internal(error_str));
    };

    //
    // See which aircraft are available to fly the route,
    //  including deadhead flights
    //
    grpc_debug!("(query_flight) timeslot pairs count {:?}", timeslot_pairs);
    let Ok(itineraries) = get_itineraries(
        &request.required_loading_time,
        &request.required_unloading_time,
        &timeslot_pairs,
        &aircraft_gaps,
        clients
    ).await else {
        let error_str = "Could not get itineraries.";
        grpc_error!("(query_flight) {}", error_str);
        return Err(Status::internal(error_str));
    };
    grpc_debug!("(query_flight) itineraries count {:?}", itineraries);

    //
    // Create draft itinerary and flight plans (in memory)
    //
    let mut response = QueryFlightResponse {
        itineraries: vec![],
    };

    let Ok(mut unconfirmed_flight_plans) = super::unconfirmed_flight_plans().lock() else {
        let error_str = "Could not get draft flight plans.";
        grpc_error!("(query_flight) {}", error_str);
        return Err(Status::internal(error_str));
    };

    let Ok(mut unconfirmed_itineraries) = super::unconfirmed_itineraries().lock() else {
        let error_str = "Could not get draft itineraries.";
        grpc_error!("(query_flight) {}", error_str);
        return Err(Status::internal(error_str));
    };

    for itinerary in itineraries.into_iter() {
        let mut flight_plans = vec![];
        for fp in itinerary {
            let flight_plan_id = Uuid::new_v4().to_string();
            flight_plans.push(FlightPlanObject {
                id: flight_plan_id.clone(),
                data: Some(fp.clone()),
            });

            unconfirmed_flight_plans.insert(flight_plan_id.clone(), fp.clone());
        }

        let itinerary_id = Uuid::new_v4().to_string();
        unconfirmed_itineraries.insert(
            itinerary_id.clone(),
            flight_plans.iter().map(|fp| fp.id.clone()).collect(),
        );

        super::cancel_itinerary_after_timeout(itinerary_id.clone());
        response.itineraries.push(Itinerary {
            itinerary_id,
            flight_plans,
        });
    }

    grpc_info!(
        "(query_flight) query_flight returning: {} flight plans.",
        &response.itineraries.len()
    );

    Ok(Response::new(response))
}

#[cfg(test)]
#[cfg(feature = "stub_backends")]
mod tests {
    // use crate::test_util::{ensure_storage_mock_data, get_vertiports_from_storage};
    // use crate::{init_logger, Config};

    // use super::*;
    // use chrono::{TimeZone, Utc};

    // #[tokio::test]
    // async fn test_get_sorted_flight_plans() {
    //     init_logger(&Config::try_from_env().unwrap_or_default());
    //     unit_test_info!("(test_get_sorted_flight_plans) start");
    //     ensure_storage_mock_data().await;
    //     let clients = get_clients().await;

    //     // our mock setup inserts only 3 flight_plans with an arrival date before "2022-10-26 14:30:00"
    //     let expected_number_returned = 3;

    //     let res = get_sorted_flight_plans(
    //         &Utc.datetime_from_str("2022-10-26 14:30:00", "%Y-%m-%d %H:%M:%S").unwrap(),
    //         &clients
    //     ).await;
    //     unit_test_debug!(
    //         "(test_get_sorted_flight_plans) flight_plans returned: {:#?}",
    //         res
    //     );

    //     assert!(res.is_ok());
    //     assert_eq!(res.unwrap().len(), expected_number_returned);
    //     unit_test_info!("(test_get_sorted_flight_plans) success");
    // }

    // #[tokio::test]
    // async fn test_query_flight() {
    //     init_logger(&Config::try_from_env().unwrap_or_default());
    //     unit_test_info!("(test_query_flight) start");
    //     ensure_storage_mock_data().await;

    //     let vertiports = get_vertiports_from_storage().await;
    //     let res = query_flight(Request::new(QueryFlightRequest {
    //         is_cargo: false,
    //         persons: None,
    //         weight_grams: None,
    //         earliest_departure_time: Some(
    //             Utc.datetime_from_str("2022-10-25 11:20:00", "%Y-%m-%d %H:%M:%S")
    //                 .unwrap()
    //                 .into(),
    //         ),
    //         latest_arrival_time: Some(
    //             Utc.datetime_from_str("2022-10-25 12:15:00", "%Y-%m-%d %H:%M:%S")
    //                 .unwrap()
    //                 .into(),
    //         ),
    //         vertiport_depart_id: vertiports[0].id.clone(),
    //         vertiport_arrive_id: vertiports[1].id.clone(),
    //     }))
    //     .await;
    //     unit_test_debug!("(test_query_flight) query_flight result: {:?}", res);
    //     assert!(res.is_ok());
    //     assert_eq!(res.unwrap().into_inner().itineraries.len(), 5);
    //     unit_test_info!("(test_query_flight) success");
    // }

    // ///4. destination vertiport is available for about 15 minutes, no other restrictions
    // /// - returns 2 flights (assuming 10 minutes needed for unloading, this can fit 2 flights
    // /// if first is exactly at the beginning of 15 minute gap and second is exactly after 5 minutes)
    // #[tokio::test]
    // async fn test_query_flight_4_dest_vertiport_tight_availability_should_return_two_flights() {
    //     init_logger(&Config::try_from_env().unwrap_or_default());
    //     unit_test_info!("(test_query_flight_4_dest_vertiport_tight_availability_should_return_two_flights) start");
    //     ensure_storage_mock_data().await;

    //     let vertiports = get_vertiports_from_storage().await;
    //     let res = query_flight(Request::new(QueryFlightRequest {
    //         is_cargo: false,
    //         persons: None,
    //         weight_grams: None,
    //         earliest_departure_time: Some(
    //             Utc.datetime_from_str("2022-10-25 14:20:00", "%Y-%m-%d %H:%M:%S")
    //                 .unwrap()
    //                 .into(),
    //         ),
    //         latest_arrival_time: Some(
    //             Utc.datetime_from_str("2022-10-25 15:10:00", "%Y-%m-%d %H:%M:%S")
    //                 .unwrap()
    //                 .into(),
    //         ),
    //         vertiport_depart_id: vertiports[2].id.clone(),
    //         vertiport_arrive_id: vertiports[1].id.clone(),
    //     }))
    //     .await
    //     .unwrap();

    //     unit_test_debug!("(test_query_flight_4_dest_vertiport_tight_availability_should_return_two_flights) query_flight result: {:#?}", res);
    //     assert_eq!(res.into_inner().itineraries.len(), 2);
    //     unit_test_info!("(test_query_flight_4_dest_vertiport_tight_availability_should_return_two_flights) success");
    // }

    // ///5. source or destination vertiport doesn't have any vertipad free for the time range
    // ///no flight plans returned
    // #[tokio::test]
    // async fn test_query_flight_5_dest_vertiport_no_availability_should_return_zero_flights() {
    //     init_logger(&Config::try_from_env().unwrap_or_default());
    //     unit_test_info!(
    //         "(test_query_flight_5_dest_vertiport_no_availability_should_return_zero_flights) start"
    //     );
    //     ensure_storage_mock_data().await;

    //     let vertiports = get_vertiports_from_storage().await;
    //     let res = query_flight(Request::new(QueryFlightRequest {
    //         is_cargo: false,
    //         persons: None,
    //         weight_grams: None,
    //         earliest_departure_time: Some(
    //             Utc.datetime_from_str("2022-10-26 14:00:00", "%Y-%m-%d %H:%M:%S")
    //                 .unwrap()
    //                 .into(),
    //         ),
    //         latest_arrival_time: Some(
    //             Utc.datetime_from_str("2022-10-26 14:40:00", "%Y-%m-%d %H:%M:%S")
    //                 .unwrap()
    //                 .into(),
    //         ),
    //         vertiport_depart_id: vertiports[1].id.clone(),
    //         vertiport_arrive_id: vertiports[0].id.clone(),
    //     }))
    //     .await;

    //     unit_test_debug!("(test_query_flight_5_dest_vertiport_no_availability_should_return_zero_flights) query_flight result: {:#?}", res);
    //     assert_eq!(
    //         res.unwrap_err()
    //             .message()
    //             .contains("No flight plans available"),
    //         true
    //     );
    //     unit_test_info!(
    //         "(test_query_flight_5_dest_vertiport_no_availability_should_return_zero_flights) success"
    //     );
    // }

    // ///6. vertiports are available but aircraft are not at the vertiport for the requested time
    // /// but at least one aircraft is IN FLIGHT to requested vertiport for that time and has availability for a next flight.
    // /// 	- skips all unavailable time slots (4) and returns only time slots from when aircraft is available (1)
    // #[tokio::test]
    // async fn test_query_flight_6_no_aircraft_at_vertiport() {
    //     init_logger(&Config::try_from_env().unwrap_or_default());
    //     unit_test_info!("(test_query_flight_6_no_aircraft_at_vertiport) start");
    //     ensure_storage_mock_data().await;

    //     let vertiports = get_vertiports_from_storage().await;
    //     let res = query_flight(Request::new(QueryFlightRequest {
    //         is_cargo: false,
    //         persons: None,
    //         weight_grams: None,
    //         earliest_departure_time: Some(
    //             Utc.datetime_from_str("2022-10-26 14:15:00", "%Y-%m-%d %H:%M:%S")
    //                 .unwrap()
    //                 .into(),
    //         ),
    //         latest_arrival_time: Some(
    //             Utc.datetime_from_str("2022-10-26 15:00:00", "%Y-%m-%d %H:%M:%S")
    //                 .unwrap()
    //                 .into(),
    //         ),
    //         vertiport_depart_id: vertiports[0].id.clone(),
    //         vertiport_arrive_id: vertiports[2].id.clone(),
    //     }))
    //     .await
    //     .unwrap()
    //     .into_inner();

    //     unit_test_debug!(
    //         "(test_query_flight_6_no_aircraft_at_vertiport) query_flight result: {:#?}",
    //         res
    //     );
    //     assert_eq!(res.itineraries.len(), 1);
    //     assert_eq!(res.itineraries[0].flight_plans.len(), 1);
    //     unit_test_info!("(test_query_flight_6_no_aircraft_at_vertiport) success");
    // }

    // /// 7. vertiports are available but aircraft are not at the vertiport for the requested time
    // /// but at least one aircraft is PARKED at other vertiport for the "requested time - N minutes"
    // #[tokio::test]
    // async fn test_query_flight_7_deadhead_flight_of_parked_vehicle() {
    //     init_logger(&Config::try_from_env().unwrap_or_default());
    //     unit_test_info!("(test_query_flight_7_deadhead_flight_of_parked_vehicle) start");
    //     ensure_storage_mock_data().await;

    //     let vertiports = get_vertiports_from_storage().await;
    //     let res = query_flight(Request::new(QueryFlightRequest {
    //         is_cargo: false,
    //         persons: None,
    //         weight_grams: None,
    //         earliest_departure_time: Some(
    //             Utc.datetime_from_str("2022-10-26 16:00:00", "%Y-%m-%d %H:%M:%S")
    //                 .unwrap()
    //                 .into(),
    //         ),
    //         latest_arrival_time: Some(
    //             Utc.datetime_from_str("2022-10-26 16:30:00", "%Y-%m-%d %H:%M:%S")
    //                 .unwrap()
    //                 .into(),
    //         ),
    //         vertiport_depart_id: vertiports[2].id.clone(),
    //         vertiport_arrive_id: vertiports[0].id.clone(),
    //     }))
    //     .await
    //     .unwrap()
    //     .into_inner();

    //     unit_test_debug!(
    //         "(test_query_flight_7_deadhead_flight_of_parked_vehicle) query_flight result: {:#?}",
    //         res
    //     );
    //     assert_eq!(res.itineraries.len(), 1);
    //     assert_eq!(res.itineraries[0].flight_plans.len(), 2);
    //     unit_test_info!("(test_query_flight_7_deadhead_flight_of_parked_vehicle) success");
    // }

    // /// 8. vertiports are available but aircraft are not at the vertiport for the requested time
    // /// but at least one aircraft is EN ROUTE to another vertiport for the "requested time - N minutes - M minutes"
    // #[tokio::test]
    // async fn test_query_flight_8_deadhead_flight_of_in_flight_vehicle() {
    //     init_logger(&Config::try_from_env().unwrap_or_default());
    //     unit_test_info!("(test_query_flight_8_deadhead_flight_of_in_flight_vehicle) start");
    //     ensure_storage_mock_data().await;

    //     let vertiports = get_vertiports_from_storage().await;
    //     let res = query_flight(Request::new(QueryFlightRequest {
    //         is_cargo: false,
    //         persons: None,
    //         weight_grams: None,
    //         earliest_departure_time: Some(
    //             Utc.datetime_from_str("2022-10-27 12:30:00", "%Y-%m-%d %H:%M:%S")
    //                 .unwrap()
    //                 .into(),
    //         ),
    //         latest_arrival_time: Some(
    //             Utc.datetime_from_str("2022-10-27 13:30:00", "%Y-%m-%d %H:%M:%S")
    //                 .unwrap()
    //                 .into(),
    //         ),
    //         vertiport_depart_id: vertiports[1].id.clone(),
    //         vertiport_arrive_id: vertiports[0].id.clone(),
    //     }))
    //     .await
    //     .unwrap()
    //     .into_inner();

    //     unit_test_debug!(
    //         "(test_query_flight_8_deadhead_flight_of_in_flight_vehicle) query_flight result: {:#?}",
    //         res
    //     );
    //     assert_eq!(res.itineraries.len(), 2);
    //     assert_eq!(res.itineraries[0].flight_plans.len(), 2);
    //     unit_test_info!("(test_query_flight_8_deadhead_flight_of_in_flight_vehicle) success");
    // }

    /* TODO: R4 refactor code and re-implement this test
    /// 9. destination vertiport is not available because of capacity
    /// - if at requested time all pads are occupied and at least one is parked (not loading/unloading),
    /// a extra flight plan should be created to move idle aircraft to the nearest unoccupied vertiport
    /// (or to preferred vertiport in hub and spoke model).
    #[tokio::test]
    async fn test_query_flight_9_deadhead_destination_flight_no_capacity_at_destination_vertiport()
    {
        init_logger(&Config::try_from_env().unwrap_or_default());
        unit_test_info!("(test_query_flight_9_deadhead_destination_flight_no_capacity_at_destination_vertiport) start");
        ensure_storage_mock_data().await;
        init_router().await;

        let vertiports = get_vertiports_from_storage().await;
        let res = query_flight(Request::new(QueryFlightRequest {
            is_cargo: false,
            persons: None,
            weight_grams: None,
            earliest_departure_time: Some(
                Utc.datetime_from_str("2022-10-27 15:10:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            latest_arrival_time: Some(
                Utc.datetime_from_str("2022-10-27 16:00:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            vertiport_depart_id: vertiports[1].id.clone(),
            vertiport_arrive_id: vertiports[3].id.clone(),
        }))
        .await
        .unwrap()
        .into_inner();

        unit_test_debug!(
            "(test_query_flight_9_deadhead_destination_flight_no_capacity_at_destination_vertiport) query_flight result: {:#?}",
            res
        );
        assert_eq!(res.itineraries.len(), 1);
        assert_eq!(res.itineraries[0].deadhead_flight_plans.len(), 1);
        unit_test_info!("(test_query_flight_9_deadhead_destination_flight_no_capacity_at_destination_vertiport) success");
    }
    */
}
