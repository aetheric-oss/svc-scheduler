//! Helper Functions for Flight Plans

use chrono::{DateTime, Utc};
use prost_wkt_types::Timestamp;
use svc_storage_client_grpc::resources::flight_plan::Data;
use svc_storage_client_grpc::GeoLineString;

/// Generates a flight plan data object from minimum required information
pub(crate) fn create_flight_plan_data(
    vehicle_id: String,
    departure_vertiport_id: String,
    arrival_vertiport_id: String,
    departure_time: DateTime<Utc>,
    arrival_time: DateTime<Utc>,
    path: GeoLineString,
) -> Data {
    Data {
        vehicle_id,
        departure_vertiport_id: Some(departure_vertiport_id),
        destination_vertiport_id: Some(arrival_vertiport_id),
        scheduled_departure: Some(departure_time.into()),
        scheduled_arrival: Some(Timestamp {
            seconds: arrival_time.timestamp(),
            nanos: arrival_time.timestamp_subsec_nanos() as i32,
        }),
        path: Some(path),
        ..Default::default() // pilot_id: "".to_string(),
                            // path: Some(path),
                            // weather_conditions: None,
                            // flight_status: 0,
                            // flight_priority: 0,
                            // departure_vertipad_id: "".to_string(),
                            // destination_vertipad_id: "".to_string(),
                            // carrier_ack: None,
                            // actual_departure: None,
                            // actual_arrival: None,
                            // flight_release_approval: None,
                            // flight_plan_submitted: None,
                            // approved_by: None,
    }
}
