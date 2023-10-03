//! Helper Functions for Flight Plans

use crate::grpc::client::GrpcClients;
use chrono::{DateTime, Utc};
use prost_wkt_types::Timestamp;
use svc_storage_client_grpc::prelude::*;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct FlightPlanSchedule {
    pub departure_vertiport_id: String,
    pub departure_vertipad_id: String,
    pub departure_time: DateTime<Utc>,
    pub arrival_vertiport_id: String,
    pub arrival_vertipad_id: String,
    pub arrival_time: DateTime<Utc>,
    pub vehicle_id: String,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FlightPlanError {
    ClientError,
    InvalidData,
}

impl std::fmt::Display for FlightPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            FlightPlanError::ClientError => write!(f, "ClientError"),
            FlightPlanError::InvalidData => write!(f, "InvalidData"),
        }
    }
}

impl TryFrom<flight_plan::Object> for FlightPlanSchedule {
    type Error = FlightPlanError;

    fn try_from(flight_plan: flight_plan::Object) -> Result<Self, Self::Error> {
        let Some(data) = flight_plan.data else {
            router_error!(
                "(try_from) Flight plan [{}] has no data.",
                flight_plan.id
            );
            return Err(FlightPlanError::InvalidData)
        };

        //
        // Must have valid departure and arrival times
        //
        let departure_time = match data.scheduled_departure {
            Some(departure_time) => departure_time.into(),
            None => {
                router_error!(
                    "(try_from) Flight plan [{}] has no scheduled departure.",
                    flight_plan.id
                );
                return Err(FlightPlanError::InvalidData);
            }
        };

        let arrival_time = match data.scheduled_arrival {
            Some(arrival_time) => arrival_time.into(),
            None => {
                router_error!(
                    "(try_from) Flight plan [{}] has no scheduled arrival.",
                    flight_plan.id
                );
                return Err(FlightPlanError::InvalidData);
            }
        };

        //
        // Must have valid departure and arrival vertiports in UUID format
        //
        let Some(departure_vertiport_id) = data.departure_vertiport_id else {
            router_error!(
                "(try_from) Flight plan [{}] has no departure vertiport.",
                flight_plan.id
            );
            return Err(FlightPlanError::InvalidData)
        };

        let departure_vertiport_id = match Uuid::parse_str(&departure_vertiport_id) {
            Ok(id) => id.to_string(),
            Err(e) => {
                router_error!(
                    "(try_from) Flight plan [{}] has invalid departure vertiport id: {}",
                    flight_plan.id,
                    e
                );
                return Err(FlightPlanError::InvalidData);
            }
        };

        let Some(arrival_vertiport_id) = data.destination_vertiport_id else {
            router_error!(
                "(try_from) Flight plan [{}] has no arrival vertiport.",
                flight_plan.id
            );
            return Err(FlightPlanError::InvalidData)
        };

        let arrival_vertiport_id = match Uuid::parse_str(&arrival_vertiport_id) {
            Ok(id) => id.to_string(),
            Err(e) => {
                router_error!(
                    "(try_from) Flight plan [{}] has invalid arrival vertiport id: {}",
                    flight_plan.id,
                    e
                );
                return Err(FlightPlanError::InvalidData);
            }
        };

        //
        // Must have a valid vehicle id in UUID format
        //
        let Ok(vehicle_id) = Uuid::parse_str(&data.vehicle_id) else {
            router_error!(
                "(try_from) Flight plan [{}] has no vehicle.",
                flight_plan.id
            );
            return Err(FlightPlanError::InvalidData)
        };

        Ok(FlightPlanSchedule {
            departure_vertiport_id,
            departure_vertipad_id: data.departure_vertipad_id,
            departure_time,
            arrival_vertiport_id,
            arrival_vertipad_id: data.destination_vertipad_id,
            arrival_time,
            vehicle_id: vehicle_id.to_string(),
        })
    }
}

/// Gets all flight plans from storage in sorted order from
///  earliest to latest arrival time
pub async fn get_sorted_flight_plans(
    latest_arrival_time: &DateTime<Utc>,
    clients: &GrpcClients,
) -> Result<Vec<FlightPlanSchedule>, FlightPlanError> {
    let latest_arrival_time: Timestamp = (*latest_arrival_time).into();

    // TODO(R4): Further filter by vehicle type, etc.
    //  With hundreds of vehicles in the air, this will be a lot of data
    //   on each call.
    let mut filter = AdvancedSearchFilter::search_less_or_equal(
        "scheduled_arrival".to_owned(),
        latest_arrival_time.to_string(),
    )
    .and_is_null("deleted_at".to_owned())
    .and_not_in(
        "flight_status".to_owned(),
        vec![
            (flight_plan::FlightStatus::Finished as i32).to_string(),
            (flight_plan::FlightStatus::Cancelled as i32).to_string(),
        ],
    );

    filter.order_by = vec![
        SortOption {
            sort_field: "vehicle_id".to_string(),
            sort_order: SortOrder::Asc as i32,
        },
        SortOption {
            sort_field: "scheduled_departure".to_owned(),
            sort_order: SortOrder::Asc as i32,
        },
    ];

    let response = match clients.storage.flight_plan.search(filter).await {
        Ok(response) => response.into_inner(),
        Err(e) => {
            router_error!(
                "(get_sorted_flight_plans) Failed to get flight plans from storage: {}",
                e
            );
            return Err(FlightPlanError::ClientError);
        }
    };

    Ok(response
        .list
        .into_iter()
        .filter_map(|fp| FlightPlanSchedule::try_from(fp).ok())
        .collect::<Vec<FlightPlanSchedule>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flight_plan_schedule_try_from() {
        let expected_departure_vertiport_id = Uuid::new_v4().to_string();
        let expected_departure_vertipad_id = Uuid::new_v4().to_string();
        let expected_arrival_vertiport_id = Uuid::new_v4().to_string();
        let expected_arrival_vertipad_id = Uuid::new_v4().to_string();
        let expected_vehicle_id = Uuid::new_v4().to_string();

        let flight_plan = flight_plan::Object {
            id: Uuid::new_v4().to_string(),
            data: Some(flight_plan::Data {
                departure_vertiport_id: Some(expected_departure_vertiport_id.clone()),
                departure_vertipad_id: expected_departure_vertipad_id.clone(),
                destination_vertiport_id: Some(expected_arrival_vertiport_id.clone()),
                destination_vertipad_id: expected_arrival_vertipad_id.clone(),
                scheduled_departure: Some(Timestamp {
                    seconds: 0,
                    nanos: 0,
                }),
                scheduled_arrival: Some(Timestamp {
                    seconds: 0,
                    nanos: 0,
                }),
                vehicle_id: expected_vehicle_id.clone(),
                ..Default::default()
            }),
            ..Default::default()
        };

        let flight_plan_schedule = FlightPlanSchedule::try_from(flight_plan).unwrap();
        assert_eq!(
            flight_plan_schedule.departure_vertiport_id,
            expected_departure_vertiport_id
        );
        assert_eq!(
            flight_plan_schedule.departure_vertipad_id,
            expected_departure_vertipad_id
        );
        assert_eq!(
            flight_plan_schedule.arrival_vertiport_id,
            expected_arrival_vertiport_id
        );
        assert_eq!(
            flight_plan_schedule.arrival_vertipad_id,
            expected_arrival_vertipad_id
        );
        assert_eq!(flight_plan_schedule.vehicle_id, expected_vehicle_id);
    }

    #[test]
    fn test_flight_plan_schedule_try_from_invalid_data() {
        let flight_plan = flight_plan::Object {
            id: "test".to_owned(),
            data: None,
            ..Default::default()
        };

        let e = FlightPlanSchedule::try_from(flight_plan).unwrap_err();
        assert_eq!(e, FlightPlanError::InvalidData);
    }
}
