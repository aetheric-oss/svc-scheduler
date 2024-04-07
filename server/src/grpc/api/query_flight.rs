//! This module contains the gRPC query_flight endpoint implementation.

use chrono::{DateTime, Duration, Utc};
use std::fmt::{Display, Formatter, Result as FmtResult};
use tonic::{Response, Status};
use uuid::Uuid;

use crate::grpc::client::get_clients;
use crate::grpc::server::grpc_server::{Itinerary, QueryFlightRequest, QueryFlightResponse};

use crate::router::flight_plan::*;
use crate::router::itinerary::calculate_itineraries;
use crate::router::schedule::*;
use crate::router::vehicle::*;
use crate::router::vertiport::*;

/// Time to block vertiport for cargo loading and takeoff
pub const LOADING_AND_TAKEOFF_TIME_SECONDS: i64 = 60;
/// Time to block vertiport for cargo unloading and landing
pub const LANDING_AND_UNLOADING_TIME_SECONDS: i64 = 60;
/// Maximum time between departure and arrival times for flight queries
pub const MAX_FLIGHT_QUERY_WINDOW_MINUTES: i64 = 720; // +/- 3 hours (6 total)

/// Sanitized version of the gRPC query
#[derive(Debug)]
struct FlightQuery {
    origin_vertiport_id: String,
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
    Internal,
}

impl Display for FlightQueryError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            FlightQueryError::InvalidVertiportId => write!(f, "Invalid vertiport ID"),
            FlightQueryError::InvalidTime => write!(f, "Invalid time"),
            FlightQueryError::TimeRangeTooLarge => write!(f, "Time range too large"),
            FlightQueryError::Internal => write!(f, "Internal error"),
        }
    }
}

impl TryFrom<QueryFlightRequest> for FlightQuery {
    type Error = FlightQueryError;

    fn try_from(request: QueryFlightRequest) -> Result<Self, Self::Error> {
        const ERROR_PREFIX: &str = "(try_from)";

        let origin_vertiport_id = Uuid::parse_str(&request.origin_vertiport_id)
            .map_err(|_| {
                grpc_error!(
                    "{} Invalid departure vertiport ID: {}",
                    ERROR_PREFIX,
                    request.origin_vertiport_id
                );
                FlightQueryError::InvalidVertiportId
            })?
            .to_string();

        let arrival_vertiport_id = Uuid::parse_str(&request.target_vertiport_id)
            .map_err(|e| {
                grpc_error!(
                    "{} Invalid departure vertiport ID {}: {e}",
                    ERROR_PREFIX,
                    request.target_vertiport_id
                );
                FlightQueryError::InvalidVertiportId
            })?
            .to_string();

        let latest_arrival_time: DateTime<Utc> = request
            .latest_arrival_time
            .ok_or_else(|| {
                grpc_warn!("{} latest arrival time not provided.", ERROR_PREFIX);
                FlightQueryError::InvalidTime
            })?
            .into();

        let earliest_departure_time: DateTime<Utc> = request
            .earliest_departure_time
            .ok_or_else(|| {
                grpc_warn!("{} earliest departure time not provided.", ERROR_PREFIX);
                FlightQueryError::InvalidTime
            })?
            .into();

        if earliest_departure_time > latest_arrival_time {
            grpc_warn!(
                "{} earliest departure time is after latest arrival time.",
                ERROR_PREFIX
            );

            return Err(FlightQueryError::InvalidTime);
        }

        // Prevent attacks where a user requests a wide flight window, resulting in a large number of
        //  calls to svc-gis for routing
        let delta = Duration::try_minutes(MAX_FLIGHT_QUERY_WINDOW_MINUTES).ok_or_else(|| {
            grpc_error!("{} error creating time delta.", ERROR_PREFIX);
            FlightQueryError::Internal
        })?;

        if (latest_arrival_time - earliest_departure_time) > delta {
            grpc_warn!("{} time range too large.", ERROR_PREFIX);
            return Err(FlightQueryError::TimeRangeTooLarge);
        }

        let delta = Duration::try_minutes(ADVANCE_NOTICE_MINUTES).ok_or_else(|| {
            grpc_error!("{} error creating time delta.", ERROR_PREFIX);
            FlightQueryError::Internal
        })?;

        if earliest_departure_time < (Utc::now() + delta) {
            grpc_warn!("{} earliest departure time is in the past, or within the next {ADVANCE_NOTICE_MINUTES} minutes.", ERROR_PREFIX);
            return Err(FlightQueryError::InvalidTime);
        }

        let required_loading_time = Duration::try_seconds(LOADING_AND_TAKEOFF_TIME_SECONDS)
            .ok_or_else(|| {
                grpc_error!("{} error creating loading time duration.", ERROR_PREFIX);
                FlightQueryError::InvalidTime
            })?;

        let required_unloading_time = Duration::try_seconds(LANDING_AND_UNLOADING_TIME_SECONDS)
            .ok_or_else(|| {
                grpc_error!("{} error creating unloading time duration.", ERROR_PREFIX);
                FlightQueryError::InvalidTime
            })?;

        Ok(FlightQuery {
            origin_vertiport_id,
            arrival_vertiport_id,
            latest_arrival_time,
            earliest_departure_time,
            // TODO(R4): Get needed loading/unloading times from request
            required_loading_time,
            required_unloading_time,
        })
    }
}

/// Finds the first possible flight for customer location, flight type and requested time.
/// TODO(R5): Return a stream of messages for live updates on query progress
pub async fn query_flight(
    request: QueryFlightRequest,
) -> Result<Response<QueryFlightResponse>, Status> {
    let request = FlightQuery::try_from(request).map_err(|e| {
        grpc_error!("(query_flight) {}", e);
        let error_str = "Invalid flight query request";
        Status::invalid_argument(error_str)
    })?;

    let timeslot = Timeslot {
        time_start: request.earliest_departure_time,
        time_end: request.latest_arrival_time,
    };

    let clients = get_clients().await;

    // Get all flight plans from this time to latest departure time (including partially fitting flight plans)
    // - this assumes that all landed flights have updated vehicle.last_vertiport_id (otherwise we would need to look in to the past)
    let existing_flight_plans: Vec<FlightPlanSchedule> =
        get_sorted_flight_plans(clients).await.map_err(|e| {
            grpc_error!("(query_flight) {}", e);
            let error_str = "Could not get existing flight plans.";
            Status::internal(error_str)
        })?;

    grpc_debug!(
        "(query_flight) found existing flight plans: {:?}",
        existing_flight_plans
    );

    //
    // TODO(R4): Determine if there's an open space for cargo on an existing flight plan
    //

    //
    // Get available timeslots for departure vertiport that are large enough to
    //  fit the required loading and takeoff time.
    //
    let timeslot_pairs = get_timeslot_pairs(
        &request.origin_vertiport_id,
        None,
        &request.arrival_vertiport_id,
        None,
        &request.required_loading_time,
        &request.required_unloading_time,
        &timeslot,
        &existing_flight_plans,
        clients,
    )
    .await
    .map_err(|e| {
        grpc_error!("(query_flight) {}", e);
        let error_str = "Could not get timeslot pairs.";
        Status::internal(error_str)
    })?;

    if timeslot_pairs.is_empty() {
        let info_str = "No routes available for the given time.";
        grpc_info!("(query_flight) {info_str}");
        return Err(Status::not_found(info_str));
    }

    //
    // Get all aircraft availabilities
    //
    let aircraft = get_aircraft(clients, None).await.map_err(|e| {
        grpc_error!("(query_flight) {}", e);
        let error_str = "Could not get aircraft.";
        Status::internal(error_str)
    })?;

    let aircraft_gaps = get_aircraft_availabilities(
        &existing_flight_plans,
        &timeslot.time_start,
        &aircraft,
        &timeslot,
    )
    .map_err(|e| {
        grpc_error!("(query_flight) {}", e);
        let error_str = "Could not get aircraft availabilities.";
        Status::internal(error_str)
    })?;

    grpc_debug!("(query_flight) aircraft gaps: {:#?}", aircraft_gaps);

    //
    // See which aircraft are available to fly the route,
    //  including deadhead flights
    //
    grpc_debug!("(query_flight) timeslot pairs count {:?}", timeslot_pairs);
    let itineraries = calculate_itineraries(
        &request.required_loading_time,
        &request.required_unloading_time,
        &timeslot_pairs,
        &aircraft_gaps,
        clients,
    )
    .await
    .map_err(|e| {
        let error_str = "Could not get itineraries";
        grpc_error!("(query_flight) {error_str}: {e}");
        Status::internal(error_str)
    })?
    .into_iter()
    .map(|flight_plans| Itinerary { flight_plans })
    .collect::<Vec<Itinerary>>();

    grpc_debug!("(query_flight) itineraries count {:?}", itineraries);

    let response = QueryFlightResponse { itineraries };
    grpc_info!(
        "(query_flight) query_flight returning: {} flight plans.",
        &response.itineraries.len()
    );

    Ok(Response::new(response))
}

#[cfg(test)]
#[cfg(feature = "stub_backends")]
mod tests {
    use super::*;
    use crate::test_util::{ensure_storage_mock_data, get_vertiports_from_storage};
    use chrono::Utc;
    use svc_storage_client_grpc::prelude::flight_plan::FlightPriority;

    #[tokio::test]
    async fn test_get_sorted_flight_plans() {
        crate::get_log_handle().await;
        ut_info!("(test_get_sorted_flight_plans) start");

        ensure_storage_mock_data().await;
        let clients = get_clients().await;

        let expected_number_returned = 10;
        let res = get_sorted_flight_plans(&clients).await;
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
            priority: FlightPriority::Low as i32,
            earliest_departure_time: None,
            latest_arrival_time: None,
            origin_vertiport_id: vertiports[0].id.clone(),
            target_vertiport_id: vertiports[1].id.clone(),
        };

        let e = FlightQuery::try_from(query.clone()).unwrap_err();
        assert_eq!(e, FlightQueryError::InvalidTime);

        // latest arrival time is less than earliest departure time
        query.earliest_departure_time = Some((Utc::now() + Duration::try_hours(4).unwrap()).into());
        query.latest_arrival_time = Some((Utc::now() + Duration::try_hours(1).unwrap()).into());

        let e = FlightQuery::try_from(query.clone()).unwrap_err();
        assert_eq!(e, FlightQueryError::InvalidTime);

        // latest arrival time is in the past
        query.latest_arrival_time = Some((Utc::now() - Duration::try_seconds(1).unwrap()).into());
        let e = FlightQuery::try_from(query.clone()).unwrap_err();
        assert_eq!(e, FlightQueryError::InvalidTime);

        // Too large of a time range
        query.earliest_departure_time = Some(Utc::now().into());
        query.latest_arrival_time = Some(
            (Utc::now() + Duration::try_minutes(MAX_FLIGHT_QUERY_WINDOW_MINUTES + 1).unwrap())
                .into(),
        );
        let e = FlightQuery::try_from(query.clone()).unwrap_err();
        assert_eq!(e, FlightQueryError::TimeRangeTooLarge);

        // Earliest departure time must also include ADVANCE_NOTICE_MINUTES
        query.earliest_departure_time = Some(Utc::now().into());
        query.latest_arrival_time = Some(
            (Utc::now() + Duration::try_minutes(MAX_FLIGHT_QUERY_WINDOW_MINUTES - 1).unwrap())
                .into(),
        );
        let e = FlightQuery::try_from(query.clone()).unwrap_err();
        assert_eq!(e, FlightQueryError::InvalidTime);

        // Valid
        query.earliest_departure_time =
            Some((Utc::now() + Duration::try_minutes(ADVANCE_NOTICE_MINUTES + 1).unwrap()).into());
        query.latest_arrival_time = Some(
            (Utc::now() + Duration::try_minutes(MAX_FLIGHT_QUERY_WINDOW_MINUTES - 1).unwrap())
                .into(),
        );
        FlightQuery::try_from(query.clone()).unwrap();

        // Invalid vertiport IDs
        query.origin_vertiport_id = "invalid".to_string();
        let e = FlightQuery::try_from(query.clone()).unwrap_err();
        assert_eq!(e, FlightQueryError::InvalidVertiportId);

        query.origin_vertiport_id = Uuid::new_v4().to_string();
        query.target_vertiport_id = "invalid".to_string();
        let e = FlightQuery::try_from(query.clone()).unwrap_err();
        assert_eq!(e, FlightQueryError::InvalidVertiportId);

        query.target_vertiport_id = Uuid::new_v4().to_string();
        FlightQuery::try_from(query.clone()).unwrap();

        ut_info!("(test_query_invalid) success");
    }
}
