//! Helper Functions for Flight Plans

use crate::grpc::client::GrpcClients;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use svc_gis_client_grpc::client::PointZ;
use svc_storage_client_grpc::prelude::*;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightPlanSchedule {
    pub origin_vertiport_id: String,
    pub origin_vertipad_id: String,
    pub origin_timeslot_start: DateTime<Utc>,
    pub origin_timeslot_end: DateTime<Utc>,
    pub target_vertiport_id: String,
    pub target_vertipad_id: String,
    pub target_timeslot_start: DateTime<Utc>,
    pub target_timeslot_end: DateTime<Utc>,
    pub vehicle_id: String,
    pub path: Option<Vec<PointZ>>,
}

impl PartialEq for FlightPlanSchedule {
    fn eq(&self, other: &Self) -> bool {
        self.origin_timeslot_start == other.origin_timeslot_start
    }
}

impl Eq for FlightPlanSchedule {}

impl PartialOrd for FlightPlanSchedule {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.origin_timeslot_start.cmp(&other.origin_timeslot_start))
    }
}

impl Ord for FlightPlanSchedule {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.origin_timeslot_start.cmp(&other.origin_timeslot_start)
    }
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

impl TryFrom<flight_plan::Data> for FlightPlanSchedule {
    type Error = FlightPlanError;

    fn try_from(data: flight_plan::Data) -> Result<Self, Self::Error> {
        //
        // Must have valid origin and target times
        //
        let path = match data.path.clone() {
            Some(p) => Some(
                p.points
                    .into_iter()
                    .map(|p| PointZ {
                        latitude: p.latitude,
                        longitude: p.longitude,
                        altitude_meters: p.altitude as f32,
                    })
                    .collect(),
            ),
            None => None,
        };

        let origin_timeslot_start: DateTime<Utc> = data
            .origin_timeslot_start
            .clone()
            .ok_or_else(|| {
                router_error!(
                    "(try_from) Flight plan has no scheduled origin start: {:?}",
                    data
                );
                FlightPlanError::InvalidData
            })?
            .into();

        let origin_timeslot_end: DateTime<Utc> = data
            .origin_timeslot_end
            .clone()
            .ok_or_else(|| {
                router_error!(
                    "(try_from) Flight plan has no scheduled origin end: {:?}",
                    data
                );
                FlightPlanError::InvalidData
            })?
            .into();

        let target_timeslot_start: DateTime<Utc> = data
            .target_timeslot_start
            .clone()
            .ok_or_else(|| {
                router_error!(
                    "(try_from) Flight plan has no scheduled target start: {:?}",
                    data
                );
                FlightPlanError::InvalidData
            })?
            .into();

        let target_timeslot_end: DateTime<Utc> = data
            .target_timeslot_end
            .clone()
            .ok_or_else(|| {
                router_error!(
                    "(try_from) Flight plan has no scheduled target end: {:?}",
                    data
                );
                FlightPlanError::InvalidData
            })?
            .into();

        if origin_timeslot_start >= target_timeslot_end {
            router_error!(
                "(try_from) Flight plan has invalid departure and arrival times: {:?}",
                data
            );
            return Err(FlightPlanError::InvalidData);
        }

        //
        // Must have valid origin and target vertiports, aircraft in UUID format
        //
        Uuid::parse_str(&data.vehicle_id).map_err(|e| {
            router_error!(
                "(try_from) Flight plan has invalid vehicle id ({}: {e}",
                data.vehicle_id
            );

            FlightPlanError::InvalidData
        })?;

        let origin_vertiport_id = data.origin_vertiport_id.clone().ok_or_else(|| {
            router_error!(
                "(try_from) Flight plan has no origin vertiport: [{:?}]",
                data
            );
            FlightPlanError::InvalidData
        })?;

        Uuid::parse_str(&origin_vertiport_id).map_err(|e| {
            router_error!(
                "(try_from) Flight plan has invalid origin vertiport ({}): [{:?}]; {}",
                origin_vertiport_id,
                data,
                e
            );
            FlightPlanError::InvalidData
        })?;

        let target_vertiport_id = data.target_vertiport_id.clone().ok_or_else(|| {
            router_error!(
                "(try_from) Flight plan has no target vertiport: [{:?}]",
                data
            );
            FlightPlanError::InvalidData
        })?;

        Uuid::parse_str(&target_vertiport_id).map_err(|e| {
            router_error!(
                "(try_from) Flight plan has invalid target vertiport ({}): [{:?}]; {}",
                target_vertiport_id,
                data,
                e
            );
            FlightPlanError::InvalidData
        })?;

        Ok(FlightPlanSchedule {
            origin_vertiport_id,
            origin_vertipad_id: data.origin_vertipad_id,
            origin_timeslot_start,
            origin_timeslot_end,
            target_vertiport_id,
            target_vertipad_id: data.target_vertipad_id,
            target_timeslot_start,
            target_timeslot_end,
            vehicle_id: data.vehicle_id.to_string(),
            path,
        })
    }
}

impl From<FlightPlanSchedule> for flight_plan::Data {
    fn from(val: FlightPlanSchedule) -> Self {
        let path = val.path.map(|p| GeoLineString {
            points: p
                .into_iter()
                .map(|p| GeoPoint {
                    latitude: p.latitude,
                    longitude: p.longitude,
                    altitude: p.altitude_meters as f64,
                })
                .collect(),
        });

        flight_plan::Data {
            origin_vertiport_id: Some(val.origin_vertiport_id),
            origin_vertipad_id: val.origin_vertipad_id,
            origin_timeslot_start: Some(val.origin_timeslot_start.into()),
            origin_timeslot_end: Some(val.origin_timeslot_end.into()),
            target_vertiport_id: Some(val.target_vertiport_id),
            target_vertipad_id: val.target_vertipad_id,
            target_timeslot_start: Some(val.target_timeslot_start.into()),
            target_timeslot_end: Some(val.target_timeslot_end.into()),
            vehicle_id: val.vehicle_id,
            path,
            ..Default::default()
        }
    }
}

impl TryFrom<flight_plan::Object> for FlightPlanSchedule {
    type Error = FlightPlanError;

    fn try_from(flight_plan: flight_plan::Object) -> Result<Self, Self::Error> {
        let data = flight_plan.data.ok_or_else(|| {
            router_error!("(try_from) Flight plan [{}] has no data.", flight_plan.id);
            FlightPlanError::InvalidData
        })?;

        Self::try_from(data)
    }
}

/// Gets flight plans from storage in sorted order from
///  earliest to latest arrival time, for the provided aircraft ids
///  or for all aircraft if none are specified.
pub async fn get_sorted_flight_plans(
    clients: &GrpcClients,
) -> Result<Vec<FlightPlanSchedule>, FlightPlanError> {
    // TODO(R4): Further filter by vehicle type, etc.
    //  With hundreds of vehicles in the air, this will be a lot of data
    //   on each call.
    let mut filter = AdvancedSearchFilter::search_is_null("deleted_at".to_owned()).and_not_in(
        "flight_status".to_owned(),
        vec![
            (flight_plan::FlightStatus::Finished as i32).to_string(),
            (flight_plan::FlightStatus::Cancelled as i32).to_string(),
        ],
    );

    filter.order_by = vec![
        SortOption {
            sort_field: "origin_timeslot_start".to_owned(),
            sort_order: SortOrder::Asc as i32,
        },
        SortOption {
            sort_field: "vehicle_id".to_string(),
            sort_order: SortOrder::Asc as i32,
        },
    ];

    let mut flight_plans = clients
        .storage
        .flight_plan
        .search(filter)
        .await
        .map_err(|e| {
            router_error!(
                "(get_sorted_flight_plans) Failed to get flight plans from storage: {}",
                e
            );
            FlightPlanError::ClientError
        })?
        .into_inner()
        .list
        .into_iter()
        .filter_map(|fp| FlightPlanSchedule::try_from(fp).ok())
        .collect::<Vec<FlightPlanSchedule>>();

    flight_plans.sort(); // should already be sorted due to the ORDER BY args to storage
    Ok(flight_plans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flight_plan_schedule_try_from() {
        let expected_origin_vertiport_id = Uuid::new_v4().to_string();
        let expected_origin_vertipad_id = Uuid::new_v4().to_string();
        let expected_target_vertiport_id = Uuid::new_v4().to_string();
        let expected_target_vertipad_id = Uuid::new_v4().to_string();
        let expected_vehicle_id = Uuid::new_v4().to_string();

        let flight_plan = flight_plan::Object {
            id: Uuid::new_v4().to_string(),
            data: Some(flight_plan::Data {
                origin_vertiport_id: Some(expected_origin_vertiport_id.clone()),
                origin_vertipad_id: expected_origin_vertipad_id.clone(),
                target_vertiport_id: Some(expected_target_vertiport_id.clone()),
                target_vertipad_id: expected_target_vertipad_id.clone(),
                origin_timeslot_start: Some(Timestamp {
                    seconds: 0,
                    nanos: 0,
                }),
                origin_timeslot_end: Some(Timestamp {
                    seconds: 1,
                    nanos: 0,
                }),
                target_timeslot_start: Some(Timestamp {
                    seconds: 2,
                    nanos: 0,
                }),
                target_timeslot_end: Some(Timestamp {
                    seconds: 3,
                    nanos: 0,
                }),
                vehicle_id: expected_vehicle_id.clone(),
                path: Some(GeoLineString {
                    points: vec![
                        GeoPoint {
                            latitude: 0.0,
                            longitude: 0.0,
                            altitude: 0.0,
                        },
                        GeoPoint {
                            latitude: 1.0,
                            longitude: 1.0,
                            altitude: 1.0,
                        },
                    ],
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        let flight_plan_schedule = FlightPlanSchedule::try_from(flight_plan).unwrap();
        assert_eq!(
            flight_plan_schedule.origin_vertiport_id,
            expected_origin_vertiport_id
        );
        assert_eq!(
            flight_plan_schedule.origin_vertipad_id,
            expected_origin_vertipad_id
        );
        assert_eq!(
            flight_plan_schedule.target_vertiport_id,
            expected_target_vertiport_id
        );
        assert_eq!(
            flight_plan_schedule.target_vertipad_id,
            expected_target_vertipad_id
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
