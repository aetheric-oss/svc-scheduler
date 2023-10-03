use crate::grpc::client::GrpcClients;
use crate::router::flight_plan::*;
use crate::router::schedule::*;
use svc_storage_client_grpc::prelude::*;

use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

/// Enum with all Aircraft types
#[derive(Debug, Copy, Clone)]
pub enum AircraftType {
    /// Cargo aircraft
    Cargo,
}

/// TODO(R4): Hardcoded for the demo. This is solely used to
///  estimate a duration of a flight.
const AVERAGE_CARGO_AIRCRAFT_CRUISE_VELOCITY_M_PER_S: f32 = 10.0;

/// Reasons for unavailable aircraft
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum VehicleError {
    ClientError,
    InvalidData,
    NoScheduleProvided,
    InvalidSchedule,
}

impl std::fmt::Display for VehicleError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            VehicleError::ClientError => write!(f, "Vehicle client error"),
            VehicleError::InvalidData => write!(f, "Vehicle data is corrupt or invalid"),
            VehicleError::NoScheduleProvided => write!(f, "Vehicle doesn't have a schedule"),
            VehicleError::InvalidSchedule => write!(f, "Vehicle has an invalid schedule"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Aircraft {
    vehicle_uuid: String,
    vehicle_calendar: Calendar,
    last_vertiport_id: String,
}

#[derive(Clone, Debug)]
pub struct Availability {
    pub timeslot: Timeslot,
    pub vertiport_id: String,
    // TODO(R4): Add vertipad occupied during this timeslot
    // vertipad_id: String,
}

impl Availability {
    fn subtract(&self, flight_plan: &FlightPlanSchedule) -> Result<Vec<Self>, VehicleError> {
        let mut slots = vec![];

        let flight_plan_timeslot = Timeslot {
            time_start: flight_plan.departure_time,
            time_end: flight_plan.arrival_time,
        };

        for timeslot in self.timeslot - flight_plan_timeslot {
            let vertiport_id = if self.timeslot.time_start < flight_plan_timeslot.time_start {
                self.vertiport_id.clone()
            } else {
                flight_plan.arrival_vertiport_id.clone()
            };

            slots.push(Availability {
                timeslot,
                vertiport_id,
            });
        }

        Ok(slots)
    }
}

impl TryFrom<vehicle::Object> for Aircraft {
    type Error = VehicleError;

    fn try_from(vehicle: vehicle::Object) -> Result<Self, VehicleError> {
        let vehicle_uuid = match Uuid::parse_str(&vehicle.id) {
            Ok(uuid) => uuid.to_string(),
            Err(e) => {
                router_error!("(try_from) Vehicle {} has invalid UUID: {}", vehicle.id, e);

                return Err(VehicleError::InvalidData);
            }
        };

        let Some(data) = vehicle.data else {
            router_error!(
                "(try_from) Vehicle doesn't have data: {:?}",
                vehicle
            );

            return Err(VehicleError::InvalidData);
        };

        let Some(last_vertiport_id) = data.last_vertiport_id else {
            router_error!(
                "(try_from) Vehicle {} doesn't have last_vertiport_id.",
                vehicle_uuid
            );

            return Err(VehicleError::InvalidData);
        };

        let last_vertiport_id = match Uuid::parse_str(&last_vertiport_id) {
            Ok(uuid) => uuid.to_string(),
            Err(e) => {
                router_error!(
                    "(try_from) Vehicle {} has invalid last_vertiport_id: {}",
                    vehicle_uuid,
                    e
                );

                return Err(VehicleError::InvalidData);
            }
        };

        let Some(calendar) = data.schedule else {
            // If vehicle doesn't have a schedule, it is not available
            //  MUST have a schedule to be a valid aircraft choice, even if the
            //  schedule is 24/7. Must be explicit.
            return Err(VehicleError::NoScheduleProvided);
        };

        let Ok(vehicle_calendar) = Calendar::from_str(&calendar) else {
            router_debug!(
                "(try_from) Invalid schedule for vehicle {}: {}",
                vehicle_uuid,
                calendar
            );

            return Err(VehicleError::InvalidSchedule);
        };

        Ok(Aircraft {
            vehicle_uuid,
            vehicle_calendar,
            last_vertiport_id,
        })
    }
}

/// Request a list of all aircraft from svc-storage
async fn get_aircraft(clients: &GrpcClients) -> Result<Vec<Aircraft>, VehicleError> {
    // TODO(R4): Private aircraft, disabled aircraft, etc. should be filtered out here
    //  This is a lot of aircraft. Possible filters:
    //   geographical area within N kilometers of request departure vertiport
    //   private aircraft
    //   disabled aircraft

    // TODO(R4): Ignore aircraft that haven't been updated recently
    // We should further limit this, but for now we'll just get all aircraft
    //  Need something to sort by, ascending distance from the
    //  departure vertiport or charge level before cutting off the list
    let filter = AdvancedSearchFilter {
        results_per_page: 1000,
        ..Default::default()
    };

    let Ok(response) = clients
        .storage
        .vehicle
        .search(filter)
        .await
    else {
            router_error!("(get_aircraft) request to svc-storage failed.");
            return Err(VehicleError::ClientError);
    };

    Ok(response
        .into_inner()
        .list
        .into_iter()
        .filter_map(|v| Aircraft::try_from(v).ok())
        .collect())
}

/// Estimates the time needed to travel between two locations including loading and unloading
/// Estimate should be rather generous to block resources instead of potentially overloading them
pub fn estimate_flight_time_seconds(distance_meters: &f32) -> Duration {
    router_debug!(
        "(estimate_flight_time_seconds) distance_meters: {}",
        *distance_meters
    );

    let aircraft = AircraftType::Cargo; // TODO(R4): Hardcoded for demo
    router_debug!("(estimate_flight_time_seconds) aircraft: {:?}", aircraft);

    match aircraft {
        AircraftType::Cargo => {
            let liftoff_duration_s: f32 = 10.0; // TODO(R4): Calculate from altitude of corridor
            let landing_duration_s: f32 = 10.0; // TODO(R4): Calculate from altitude of corridor

            let cruise_duration_s: f32 =
                *distance_meters / AVERAGE_CARGO_AIRCRAFT_CRUISE_VELOCITY_M_PER_S;

            Duration::seconds((liftoff_duration_s + cruise_duration_s + landing_duration_s) as i64)
        }
    }
}

/// From an aircraft's calendar and list of busy timeslots, determine
///  the aircraft's availability and location at a given time.
fn get_aircraft_availability(
    aircraft: Aircraft,
    aircraft_schedule: &[FlightPlanSchedule],
) -> Vec<Availability> {
    // Get timeslots from vehicle's general calendar
    //  e.g. 8AM to 12PM, 2PM to 6PM
    let mut availability = aircraft
        .vehicle_calendar
        .to_timeslots(
            // 2 hours before earliest departure time
            &(Utc::now() - Duration::hours(2)),
            // 2 hours after latest arrival time
            &(Utc::now() + Duration::hours(2)),
        )
        .into_iter()
        .map(|slot| {
            Availability {
                timeslot: slot,
                vertiport_id: aircraft.last_vertiport_id.clone(),
                // vertipad_id: aircraft.last_vertipad_id.clone(),
            }
        })
        .collect::<Vec<Availability>>();

    // Existing flight plans modify availability
    for fp in aircraft_schedule {
        availability = availability
            .into_iter()
            // Remove any slots that overlap with the occupied slot
            .filter_map(|availability| availability.subtract(fp).ok())
            .flatten()
            .collect::<Vec<Availability>>()
    }

    availability
}

/// Build out a list of available aircraft (and their scheduled locations)
///  given a list of existing flight plans.
pub async fn get_aircraft_gaps(
    existing_flight_plans: &[FlightPlanSchedule],
    clients: &GrpcClients,
) -> Result<HashMap<String, Vec<Availability>>, VehicleError> {
    let aircraft: Vec<Aircraft> = get_aircraft(clients).await?;
    let mut aircraft_schedules = aircraft
        .iter()
        .map(|a| (a.vehicle_uuid.clone(), vec![]))
        .collect::<HashMap<String, Vec<FlightPlanSchedule>>>();

    // Group flight plans by vehicle_id
    existing_flight_plans.iter().for_each(|fp| {
        // only push flight plans for aircraft that we have in our list
        // don't want to schedule new flights for removed aircraft
        if let Some(schedule) = aircraft_schedules.get_mut(&fp.vehicle_id) {
            schedule.push(fp.clone());
        } else {
            router_warn!(
                "(get_aircraft_gaps) Flight plan for unknown aircraft: {}",
                fp.vehicle_id
            );
        }
    });

    // Convert to a hashmap of vehicle_id to list of availabilities
    let mut gaps = HashMap::new();
    for vehicle in aircraft.into_iter() {
        let Some(schedule) = aircraft_schedules.get_mut(&vehicle.vehicle_uuid) else {
            router_warn!(
                "(get_aircraft_gaps) Flight plan for unknown aircraft: {}",
                vehicle.vehicle_uuid
            );

            continue;
        };

        gaps.insert(
            vehicle.vehicle_uuid.clone(),
            get_aircraft_availability(vehicle, schedule),
        );
    }

    Ok(gaps)
}
