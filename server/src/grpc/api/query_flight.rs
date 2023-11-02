//! This module contains the gRPC query_flight endpoint implementation.

use chrono::{DateTime, Duration, Utc};
use std::collections::BinaryHeap;
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
#[derive(Debug)]
struct FlightQuery {
    departure_vertiport_id: String,
    arrival_vertiport_id: String,
    earliest_departure_time: DateTime<Utc>,
    latest_arrival_time: DateTime<Utc>,
    required_loading_time: Duration,
    required_unloading_time: Duration,
}

/// Error type for FlightQuery
#[derive(Debug, Clone, Copy, PartialEq)]
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
        const ERROR_PREFIX: &str = "(try_from)";

        let departure_vertiport_id = match Uuid::parse_str(&request.vertiport_depart_id) {
            Ok(id) => id.to_string(),
            _ => {
                grpc_error!(
                    "{} Invalid departure vertiport ID: {}",
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
                    "{} Invalid departure vertiport ID: {}",
                    ERROR_PREFIX,
                    request.vertiport_arrive_id
                );
                return Err(FlightQueryError::InvalidVertiportId);
            }
        };

        let Some(latest_arrival_time) = request.latest_arrival_time.clone() else {
            grpc_warn!("{} latest arrival time not provided.", ERROR_PREFIX);
            return Err(FlightQueryError::InvalidTime);
        };

        let Some(earliest_departure_time) = request.earliest_departure_time else {
            grpc_warn!("{} earliest departure time not provided.", ERROR_PREFIX);
            return Err(FlightQueryError::InvalidTime);
        };

        let latest_arrival_time: DateTime<Utc> = latest_arrival_time.into();
        let earliest_departure_time: DateTime<Utc> = earliest_departure_time.into();

        if earliest_departure_time > latest_arrival_time {
            grpc_warn!(
                "{} earliest departure time is after latest arrival time.",
                ERROR_PREFIX
            );
            return Err(FlightQueryError::InvalidTime);
        }

        // Prevent attacks where a user requests a wide flight window, resulting in a large number of
        //  calls to svc-gis for routing
        if latest_arrival_time - earliest_departure_time
            > Duration::minutes(MAX_FLIGHT_QUERY_WINDOW_MINUTES)
        {
            grpc_warn!("{} time range too large.", ERROR_PREFIX);
            return Err(FlightQueryError::TimeRangeTooLarge);
        }

        if latest_arrival_time < Utc::now() {
            grpc_warn!("{} latest arrival time is in the past.", ERROR_PREFIX);
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
    let mut existing_flight_plans: BinaryHeap<FlightPlanSchedule> =
        match get_sorted_flight_plans(&request.latest_arrival_time, clients).await {
            Ok(plans) => plans,
            Err(e) => {
                let error_str = "Could not get existing flight plans.";
                grpc_error!("(query_flight) {} {}", error_str, e);
                return Err(Status::internal(error_str));
            }
        };

    // Add draft flight plans to the "existing" flight plans
    {
        let Ok(draft_flight_plans) = super::unconfirmed_flight_plans().lock() else {
            let error_str = "Could not get draft flight plans.";
            grpc_error!("(query_flight) {}", error_str);
            return Err(Status::internal(error_str));
        };

        draft_flight_plans
            .iter()
            .filter_map(|(id, fp)| {
                let object = FlightPlanObject {
                    id: id.clone(),
                    data: Some(fp.clone()),
                };

                match FlightPlanSchedule::try_from(object) {
                    Ok(mut schedule) => {
                        schedule.draft = true;
                        Some(schedule)
                    }
                    Err(e) => {
                        grpc_error!(
                            "(query_flight) Could not parse draft flight plan with id {}: {}",
                            &id,
                            e
                        );
                        None
                    }
                }
            })
            .collect::<Vec<FlightPlanSchedule>>()
            .into_iter()
            .for_each(|fp| existing_flight_plans.push(fp));
    }

    let existing_flight_plans = existing_flight_plans.into_sorted_vec();

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
    )
    .await
    else {
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
    let Ok(aircraft) = get_aircraft(clients).await else {
        let error_str = "Could not get aircraft.";
        grpc_error!("(query_flight) {}", error_str);
        return Err(Status::internal(error_str));
    };

    let aircraft_gaps = get_aircraft_gaps(&existing_flight_plans, &aircraft, &timeslot);

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
        clients,
    )
    .await
    else {
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
            id: itinerary_id,
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
    use crate::test_util::{ensure_storage_mock_data, get_vertiports_from_storage};

    use super::*;
    use chrono::{TimeZone, Utc};

    #[tokio::test]
    async fn test_get_sorted_flight_plans() {
        crate::get_log_handle().await;
        ut_info!("(test_get_sorted_flight_plans) start");

        ensure_storage_mock_data().await;
        let clients = get_clients().await;

        // our mock setup inserts only 3 flight_plans with an arrival date before "2022-10-26 14:30:00"
        let expected_number_returned = 3;

        let chrono::LocalResult::Single(date) = Utc.with_ymd_and_hms(2022, 10, 26, 14, 30, 0)
        else {
            panic!();
        };

        let res = get_sorted_flight_plans(&date, &clients).await;
        ut_debug!(
            "(test_get_sorted_flight_plans) flight_plans returned: {:#?}",
            res
        );

        assert!(res.is_ok());
        assert_eq!(res.unwrap().len(), expected_number_returned);
        ut_info!("(test_get_sorted_flight_plans) success");
    }

    #[tokio::test]
    #[cfg(feature = "stub_backends")]
    async fn test_query_invalid() {
        crate::get_log_handle().await;
        ut_info!("(test_query_invalid) start");

        let vertiports = get_vertiports_from_storage().await;
        let mut query = QueryFlightRequest {
            is_cargo: true,
            persons: None,
            weight_grams: Some(10),
            earliest_departure_time: None,
            latest_arrival_time: None,
            vertiport_depart_id: vertiports[0].id.clone(),
            vertiport_arrive_id: vertiports[1].id.clone(),
        };

        let e = FlightQuery::try_from(query.clone()).unwrap_err();
        assert_eq!(e, FlightQueryError::InvalidTime);

        // latest arrival time is less than earliest departure time
        query.earliest_departure_time = Some((Utc::now() + Duration::hours(4)).into());
        query.latest_arrival_time = Some((Utc::now() + Duration::hours(1)).into());

        let e = FlightQuery::try_from(query.clone()).unwrap_err();
        assert_eq!(e, FlightQueryError::InvalidTime);

        // latest arrival time is in the past
        query.latest_arrival_time = Some((Utc::now() - Duration::seconds(1)).into());
        let e = FlightQuery::try_from(query.clone()).unwrap_err();
        assert_eq!(e, FlightQueryError::InvalidTime);

        // Too large of a time range
        query.earliest_departure_time = Some(Utc::now().into());
        query.latest_arrival_time =
            Some((Utc::now() + Duration::minutes(MAX_FLIGHT_QUERY_WINDOW_MINUTES + 1)).into());
        let e = FlightQuery::try_from(query.clone()).unwrap_err();
        assert_eq!(e, FlightQueryError::TimeRangeTooLarge);

        query.earliest_departure_time = Some(Utc::now().into());
        query.latest_arrival_time =
            Some((Utc::now() + Duration::minutes(MAX_FLIGHT_QUERY_WINDOW_MINUTES - 1)).into());
        FlightQuery::try_from(query.clone()).unwrap();

        // Invalid vertiport IDs
        query.vertiport_depart_id = "invalid".to_string();
        let e = FlightQuery::try_from(query.clone()).unwrap_err();
        assert_eq!(e, FlightQueryError::InvalidVertiportId);

        query.vertiport_depart_id = Uuid::new_v4().to_string();
        query.vertiport_arrive_id = "invalid".to_string();
        let e = FlightQuery::try_from(query.clone()).unwrap_err();
        assert_eq!(e, FlightQueryError::InvalidVertiportId);

        query.vertiport_arrive_id = Uuid::new_v4().to_string();
        FlightQuery::try_from(query.clone()).unwrap();

        ut_info!("(test_query_invalid) success");
    }
}
