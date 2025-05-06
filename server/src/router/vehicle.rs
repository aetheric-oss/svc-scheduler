use crate::grpc::client::GrpcClients;
use crate::router::flight_plan::*;
use crate::router::schedule::*;
use svc_storage_client_grpc::prelude::*;

use lib_common::time::{DateTime, Duration, Utc};
use lib_common::uuid::Uuid;
use std::collections::HashMap;
use std::str::FromStr;

/// Enum with all Aircraft types
#[derive(Debug, Copy, Clone)]
pub enum AircraftType {
    /// Cargo aircraft
    Cargo,
}

/// TODO(R5): Hardcoded for the demo. This is solely used to
///  estimate a duration of a flight.
const AVERAGE_CARGO_AIRCRAFT_CRUISE_VELOCITY_M_PER_S: f32 = 10.0;

/// Reasons for unavailable aircraft
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum VehicleError {
    /// Error from the vehicle client
    ClientError,

    /// Vehicle data is corrupt or invalid
    Data,

    /// Vehicle has an invalid UUID
    VehicleId,

    /// Vehicle doesn't have a hangar_id
    HangarId,

    /// Vehicle doesn't have a hangar_bay_id
    HangarBayId,

    /// Vehicle doesn't have a schedule
    NoSchedule,

    /// Vehicle has an invalid schedule
    Schedule,

    /// Internal error
    Internal,
}

impl std::fmt::Display for VehicleError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            VehicleError::ClientError => write!(f, "Vehicle client error"),
            VehicleError::Data => write!(f, "Vehicle data is corrupt or invalid"),
            VehicleError::VehicleId => write!(f, "Vehicle has an invalid UUID"),
            VehicleError::HangarId => write!(f, "Vehicle doesn't have a hangar_id"),
            VehicleError::HangarBayId => write!(f, "Vehicle doesn't have a hangar_bay_id"),
            VehicleError::NoSchedule => write!(f, "Vehicle doesn't have a schedule"),
            VehicleError::Schedule => write!(f, "Vehicle has an invalid schedule"),
            VehicleError::Internal => write!(f, "Internal error"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Aircraft {
    vehicle_uuid: String,
    vehicle_calendar: Calendar,
    hangar_id: String,
    hangar_bay_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Availability {
    pub timeslot: Timeslot,
    pub vertiport_id: String,
    pub vertipad_id: String,
}

impl Availability {
    fn subtract(&self, flight_plan: &FlightPlanSchedule) -> Vec<Self> {
        let mut slots = vec![];

        let Ok(flight_plan_timeslot) = Timeslot::new(
            flight_plan.origin_timeslot_start,
            flight_plan.target_timeslot_start,
        ) else {
            router_error!(
                "Invalid flight plan timeslot, returning no availabilities: {:?} {:?}",
                flight_plan.origin_timeslot_start,
                flight_plan.target_timeslot_start
            );
            return slots;
        };

        let timeslots = self.timeslot - flight_plan_timeslot;
        router_debug!(
            "(Availability::subtract) self: {:?}, fp: {:?}",
            self.timeslot,
            flight_plan_timeslot
        );
        router_debug!("result: {:?}", timeslots);
        for timeslot in timeslots {
            let (vertiport_id, vertipad_id) =
                if timeslot.time_start() < flight_plan_timeslot.time_start() {
                    (self.vertiport_id.clone(), self.vertipad_id.clone())
                } else {
                    (
                        flight_plan.target_vertiport_id.clone(),
                        flight_plan.target_vertipad_id.clone(),
                    )
                };

            slots.push(Availability {
                timeslot,
                vertiport_id,
                vertipad_id,
            });
        }

        slots
    }
}

impl TryFrom<vehicle::Object> for Aircraft {
    type Error = VehicleError;

    fn try_from(vehicle: vehicle::Object) -> Result<Self, VehicleError> {
        let vehicle_uuid = Uuid::parse_str(&vehicle.id)
            .map_err(|e| {
                router_error!("Vehicle {} has invalid UUID: {}", vehicle.id, e);
                VehicleError::VehicleId
            })?
            .to_string();

        let data = vehicle.data.as_ref().ok_or_else(|| {
            router_error!("Vehicle doesn't have data: {:?}", vehicle);
            VehicleError::Data
        })?;

        let hangar_id = data.hangar_id.clone().ok_or_else(|| {
            router_error!("Vehicle {} doesn't have hangar_id.", vehicle_uuid);
            VehicleError::HangarId
        })?;

        let hangar_id = Uuid::parse_str(&hangar_id)
            .map_err(|e| {
                router_error!("Vehicle {} has invalid hangar_id: {}", vehicle_uuid, e);

                VehicleError::HangarId
            })?
            .to_string();

        let hangar_bay_id = data.hangar_bay_id.clone().ok_or_else(|| {
            router_error!("Vehicle {} doesn't have hangar_bay_id.", vehicle_uuid);
            VehicleError::HangarBayId
        })?;

        let hangar_bay_id = Uuid::parse_str(&hangar_bay_id)
            .map_err(|e| {
                router_error!("Vehicle {} has invalid hangar_bay_id: {}", vehicle_uuid, e);

                VehicleError::HangarBayId
            })?
            .to_string();

        let calendar = data.schedule.clone().ok_or_else(|| {
            // If vehicle doesn't have a schedule, it is not available
            //  MUST have a schedule to be a valid aircraft choice, even if the
            //  schedule is 24/7. Must be explicit.
            router_error!("Vehicle {} doesn't have a schedule.", vehicle_uuid);
            VehicleError::NoSchedule
        })?;

        let vehicle_calendar = Calendar::from_str(&calendar).map_err(|e| {
            router_debug!(
                "Invalid schedule for vehicle {} {}; {e}",
                vehicle_uuid,
                calendar
            );

            VehicleError::Schedule
        })?;

        Ok(Aircraft {
            vehicle_uuid,
            vehicle_calendar,
            hangar_id,
            hangar_bay_id,
        })
    }
}

/// Request a list of all aircraft from svc-storage
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) needs running backend, integration tests
pub async fn get_aircraft(
    clients: &GrpcClients,
    aircraft_id: Option<String>,
) -> Result<Vec<Aircraft>, VehicleError> {
    // TODO(R5): Private aircraft, disabled aircraft, etc. should be filtered out here
    //  This is a lot of aircraft. Possible filters:
    //   geographical area within N kilometers of request origin vertiport
    //   private aircraft
    //   disabled aircraft

    // TODO(R5): Ignore aircraft that haven't been updated recently
    // We should further limit this, but for now we'll just get all aircraft
    //  Need something to sort by, ascending distance from the
    //  departure vertiport or charge level before cutting off the list

    let mut filter = match aircraft_id {
        Some(id) => AdvancedSearchFilter::search_equals("vehicle_id".to_string(), id.to_string()),
        None => AdvancedSearchFilter::default(),
    };

    filter.results_per_page = 1000;

    let response = clients
        .storage
        .vehicle
        .search(filter)
        .await
        .map_err(|e| {
            router_error!("request to svc-storage failed: {e}");
            VehicleError::ClientError
        })?
        .into_inner()
        .list
        .into_iter()
        .filter_map(|v| Aircraft::try_from(v).ok())
        .collect();

    Ok(response)
}

/// Estimates the time needed to travel between two locations including loading and unloading
/// Estimate should be rather generous to block resources instead of potentially overloading them
pub fn estimate_flight_time_seconds(distance_meters: &f64) -> Result<Duration, VehicleError> {
    router_debug!("distance_meters: {}", *distance_meters);

    let aircraft = AircraftType::Cargo; // TODO(R5): Hardcoded for demo
    router_debug!("aircraft: {:?}", aircraft);

    match aircraft {
        AircraftType::Cargo => {
            let liftoff_duration_s: f32 = 10.0; // TODO(R5): Calculate from altitude of corridor
            let landing_duration_s: f32 = 10.0; // TODO(R5): Calculate from altitude of corridor

            let cruise_duration_s: f32 =
                (*distance_meters as f32) / AVERAGE_CARGO_AIRCRAFT_CRUISE_VELOCITY_M_PER_S;

            let total_duration_s: f32 = liftoff_duration_s + cruise_duration_s + landing_duration_s;
            Duration::try_milliseconds((total_duration_s * 1000.0) as i64).ok_or_else(|| {
                router_error!("error creating time delta.");
                VehicleError::Internal
            })
        }
    }
}

/// Build out a list of available aircraft (and their scheduled locations)
///  given a list of existing flight plans.
pub fn get_aircraft_availabilities(
    existing_flight_plans: &[FlightPlanSchedule],
    earliest_departure_time: &DateTime<Utc>,
    aircraft: &[Aircraft],
    timeslot: &Timeslot,
) -> Result<HashMap<String, Vec<Availability>>, VehicleError> {
    router_debug!("aircraft: {:?}", aircraft);
    let deadhead_padding: Duration = Duration::try_hours(2).ok_or_else(|| {
        router_error!("error creating time delta.");
        VehicleError::Internal
    })?;

    let mut aircraft_availabilities: HashMap<String, Vec<Availability>> = HashMap::new();
    for a in aircraft.iter() {
        let hangar_id = a.hangar_id.clone();
        let hangar_bay_id = a.hangar_bay_id.clone();
        // Aircraft also needs time to deadhead before and after primary flight
        // Base availability from vehicle calendar
        a.vehicle_calendar
            .to_timeslots(
                &(timeslot.time_start() - deadhead_padding),
                &(timeslot.time_end() + deadhead_padding),
            )
            .map_err(|e| {
                router_error!("error creating timeslots: {}", e);
                VehicleError::Internal
            })?
            .into_iter()
            .for_each(|timeslot| {
                // ignore timeslots that are in the past
                // the user will need time to select an itinerary, by the time they select
                // the itinerary start should not be in the past. Add delta to compensate
                let Ok(tmp) = Timeslot::new(
                    timeslot.time_start().max(*earliest_departure_time),
                    timeslot.time_end(),
                ) else {
                    return;
                };

                aircraft_availabilities
                    .entry(a.vehicle_uuid.clone())
                    .or_default()
                    .push(Availability {
                        timeslot: tmp,
                        vertiport_id: hangar_id.clone(),
                        vertipad_id: hangar_bay_id.clone(),
                    });
            });
    }

    router_debug!(
        "aircraft base availabilities: {:?}",
        aircraft_availabilities
    );

    // Group flight plans by vehicle_id
    existing_flight_plans.iter().for_each(|fp| {
        // only push flight plans for aircraft that we have in our list
        // don't want to schedule new flights for removed aircraft
        if let Some(availabilities) = aircraft_availabilities.get_mut(&fp.vehicle_id) {
            *availabilities = availabilities
                .iter()
                .flat_map(|a| a.subtract(fp))
                .collect::<Vec<Availability>>();
        } else {
            router_warn!("Flight plan for unknown aircraft: {}", fp.vehicle_id);
        }
    });

    router_debug!(
        "aircraft availabilities after flight plans: {:?}",
        aircraft_availabilities
    );

    Ok(aircraft_availabilities)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lib_common::time::{Datelike, LocalResult, TimeZone, Utc};

    #[test]
    fn test_subtract_flight_plan() {
        let vertiport_start_id = Uuid::new_v4().to_string();
        let vertipad_start_id = Uuid::new_v4().to_string();
        let vertiport_middle_id = Uuid::new_v4().to_string();
        let vertipad_middle_id = Uuid::new_v4().to_string();
        let aircraft_id = Uuid::new_v4().to_string();

        let year = Utc::now().year() + 1;
        let LocalResult::Single(dt_start) = Utc.with_ymd_and_hms(year, 10, 20, 0, 0, 0) else {
            panic!();
        };

        let availability = Availability {
            timeslot: Timeslot::new(dt_start, dt_start + Duration::try_hours(2).unwrap()).unwrap(),
            vertiport_id: vertiport_start_id.clone(),
            vertipad_id: vertipad_start_id.clone(),
        };

        let flight_plans = vec![
            FlightPlanSchedule {
                vehicle_id: aircraft_id.clone(),
                origin_vertiport_id: vertiport_start_id.clone(),
                origin_vertipad_id: vertipad_start_id.clone(),
                target_vertiport_id: vertiport_middle_id.clone(),
                target_vertipad_id: vertipad_middle_id.clone(),
                origin_timeslot_start: dt_start + Duration::try_minutes(10).unwrap(),
                origin_timeslot_end: dt_start + Duration::try_minutes(10).unwrap(),
                target_timeslot_start: dt_start + Duration::try_minutes(20).unwrap(),
                target_timeslot_end: dt_start + Duration::try_minutes(20).unwrap(),
                waypoints: Some(vec![]),
            },
            FlightPlanSchedule {
                vehicle_id: aircraft_id.clone(),
                origin_vertiport_id: vertiport_middle_id.clone(),
                origin_vertipad_id: vertipad_middle_id.clone(),
                target_vertiport_id: vertiport_start_id.clone(),
                target_vertipad_id: vertipad_start_id.clone(),
                origin_timeslot_start: dt_start + Duration::try_minutes(25).unwrap(),
                origin_timeslot_end: dt_start + Duration::try_minutes(25).unwrap(),
                target_timeslot_start: dt_start + Duration::try_minutes(35).unwrap(),
                target_timeslot_end: dt_start + Duration::try_minutes(35).unwrap(),
                waypoints: Some(vec![]),
            },
        ];

        let result = availability.subtract(&flight_plans[0]);
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            Availability {
                timeslot: Timeslot::new(dt_start, flight_plans[0].origin_timeslot_start).unwrap(),
                vertiport_id: vertiport_start_id.clone(),
                vertipad_id: vertipad_start_id.clone()
            }
        );
        assert_eq!(
            result[1],
            Availability {
                timeslot: Timeslot::new(
                    flight_plans[0].target_timeslot_start,
                    dt_start + Duration::try_hours(2).unwrap()
                )
                .unwrap(),
                vertiport_id: vertiport_middle_id.clone(),
                vertipad_id: vertipad_middle_id.clone()
            }
        );

        let result = availability.subtract(&flight_plans[1]);
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            Availability {
                timeslot: Timeslot::new(dt_start, flight_plans[1].origin_timeslot_start).unwrap(),
                vertiport_id: vertiport_start_id.clone(),
                vertipad_id: vertipad_start_id.clone()
            }
        );
        assert_eq!(
            result[1],
            Availability {
                timeslot: Timeslot::new(
                    flight_plans[1].target_timeslot_start,
                    dt_start + Duration::try_hours(2).unwrap()
                )
                .unwrap(),
                vertiport_id: vertiport_start_id.clone(),
                vertipad_id: vertipad_start_id.clone()
            }
        );

        //
        // Multiple flight plans
        //
        let mut availabilities = vec![availability];
        for fp in &flight_plans {
            availabilities = availabilities
                .iter_mut()
                .flat_map(|availability| availability.subtract(&fp))
                .collect::<Vec<Availability>>();
        }

        assert_eq!(availabilities.len(), 3);
        assert_eq!(
            availabilities[0],
            Availability {
                timeslot: Timeslot::new(dt_start, flight_plans[0].origin_timeslot_start).unwrap(),
                vertiport_id: vertiport_start_id.clone(),
                vertipad_id: vertipad_start_id.clone()
            }
        );
        assert_eq!(
            availabilities[1],
            Availability {
                timeslot: Timeslot::new(
                    flight_plans[0].target_timeslot_start,
                    flight_plans[1].origin_timeslot_start
                )
                .unwrap(),
                vertiport_id: vertiport_middle_id.clone(),
                vertipad_id: vertipad_middle_id.clone()
            }
        );
        assert_eq!(
            availabilities[2],
            Availability {
                timeslot: Timeslot::new(
                    flight_plans[1].target_timeslot_start,
                    dt_start + Duration::try_hours(2).unwrap()
                )
                .unwrap(),
                vertiport_id: vertiport_start_id.clone(),
                vertipad_id: vertipad_start_id.clone()
            }
        );
    }

    #[test]
    fn test_get_aircraft_availabilities() {
        let vehicle_duration_hours = 3;
        let schedule = Calendar::from_str(&format!(
            "DTSTART:20230920T000000Z;DURATION:PT{vehicle_duration_hours}H\n\
        RRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR,SA,SU"
        ))
        .unwrap();

        let year = Utc::now().year() + 1;
        let LocalResult::Single(dt_start) = Utc.with_ymd_and_hms(year, 10, 20, 0, 0, 0) else {
            panic!();
        };

        let timeslots = schedule
            .clone()
            .to_timeslots(&dt_start, &(dt_start + Duration::try_hours(2).unwrap()))
            .unwrap();
        assert_eq!(timeslots.len(), 1);
        assert_eq!(
            timeslots[0],
            Timeslot::new(dt_start, dt_start + Duration::try_hours(2).unwrap()).unwrap()
        );

        let vertiport_start_id = Uuid::new_v4().to_string();
        let vertipad_start_id = Uuid::new_v4().to_string();
        let vertiport_middle_id = Uuid::new_v4().to_string();
        let vertipad_middle_id = Uuid::new_v4().to_string();
        let aircraft_id = Uuid::new_v4().to_string();

        let aircraft = vec![Aircraft {
            vehicle_uuid: aircraft_id.clone(),
            vehicle_calendar: schedule,
            hangar_id: vertiport_start_id.clone(),
            hangar_bay_id: vertipad_start_id.clone(),
        }];

        let timeslot = Timeslot::new(dt_start, dt_start + Duration::try_hours(2).unwrap()).unwrap();

        let flight_plans = vec![
            FlightPlanSchedule {
                vehicle_id: aircraft_id.clone(),
                origin_vertiport_id: vertiport_start_id.clone(),
                origin_vertipad_id: vertipad_start_id.clone(),
                target_vertiport_id: vertiport_middle_id.clone(),
                target_vertipad_id: vertipad_middle_id.clone(),
                origin_timeslot_start: dt_start + Duration::try_minutes(10).unwrap(),
                origin_timeslot_end: dt_start + Duration::try_minutes(10).unwrap(),
                target_timeslot_start: dt_start + Duration::try_minutes(20).unwrap(),
                target_timeslot_end: dt_start + Duration::try_minutes(20).unwrap(),
                waypoints: Some(vec![]),
            },
            FlightPlanSchedule {
                vehicle_id: aircraft_id.clone(),
                origin_vertiport_id: vertiport_middle_id.clone(),
                origin_vertipad_id: vertipad_middle_id.clone(),
                target_vertiport_id: vertiport_start_id.clone(),
                target_vertipad_id: vertipad_start_id.clone(),
                origin_timeslot_start: dt_start + Duration::try_minutes(25).unwrap(),
                origin_timeslot_end: dt_start + Duration::try_minutes(25).unwrap(),
                target_timeslot_start: dt_start + Duration::try_minutes(35).unwrap(),
                target_timeslot_end: dt_start + Duration::try_minutes(35).unwrap(),
                waypoints: Some(vec![]),
            },
        ];

        let mut gaps = get_aircraft_availabilities(
            &flight_plans,
            &timeslot.time_start(),
            &aircraft,
            &timeslot,
        )
        .unwrap();

        println!("gaps: {:?}", gaps);

        assert_eq!(gaps.len(), 1);
        let gaps = gaps.get_mut(&aircraft_id).unwrap();

        println!("gaps: {:?}", gaps);

        assert_eq!(gaps.len(), 3);
        gaps.sort_by(|a, b| b.timeslot.time_start().cmp(&a.timeslot.time_start()));
        assert_eq!(
            gaps.pop().unwrap(),
            Availability {
                timeslot: Timeslot::new(dt_start, flight_plans[0].origin_timeslot_start).unwrap(),
                vertiport_id: vertiport_start_id.clone(),
                vertipad_id: vertipad_start_id.clone()
            }
        );

        assert_eq!(
            gaps.pop().unwrap(),
            Availability {
                timeslot: Timeslot::new(
                    flight_plans[0].target_timeslot_start,
                    flight_plans[1].origin_timeslot_start
                )
                .unwrap(),
                vertiport_id: vertiport_middle_id.clone(),
                vertipad_id: vertipad_middle_id.clone()
            }
        );

        assert_eq!(
            gaps.pop().unwrap(),
            Availability {
                timeslot: Timeslot::new(
                    flight_plans[1].target_timeslot_start,
                    // see 'deadhead_padding' in the function
                    // the vehicle schedule in this example is 3 hours long, less than the deadhead padding,
                    //  so the end time is the end of the vehicle schedule in this case
                    dt_start + Duration::try_hours(vehicle_duration_hours).unwrap()
                )
                .unwrap(),
                vertiport_id: vertiport_start_id,
                vertipad_id: vertipad_start_id
            }
        );
    }

    #[test]
    fn test_vehicle_error_display() {
        assert_eq!(
            format!("{}", VehicleError::ClientError),
            "Vehicle client error"
        );
        assert_eq!(
            format!("{}", VehicleError::Data),
            "Vehicle data is corrupt or invalid"
        );
        assert_eq!(
            format!("{}", VehicleError::VehicleId),
            "Vehicle has an invalid UUID"
        );
        assert_eq!(
            format!("{}", VehicleError::HangarId),
            "Vehicle doesn't have a hangar_id"
        );
        assert_eq!(
            format!("{}", VehicleError::HangarBayId),
            "Vehicle doesn't have a hangar_bay_id"
        );
        assert_eq!(
            format!("{}", VehicleError::NoSchedule),
            "Vehicle doesn't have a schedule"
        );
        assert_eq!(
            format!("{}", VehicleError::Schedule),
            "Vehicle has an invalid schedule"
        );
        assert_eq!(format!("{}", VehicleError::Internal), "Internal error");
    }

    #[test]
    fn test_try_from_vehicle_object_aircraft() {
        const CAL_STR: &str = "DTSTART:20221020T180000Z;DURATION:PT14H
            RRULE:FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR";

        let vehicle_data = vehicle::Data {
            vehicle_model_id: Uuid::new_v4().to_string(),
            serial_number: format!("S-MOCK-{:0>8}", 12345678),
            registration_number: format!("N-DEMO-{:0>8}", 12345678),
            description: Some("Demo vehicle filled with Mock data".to_owned()),
            asset_group_id: None,
            schedule: Some(CAL_STR.to_owned()),
            hangar_id: Some(Uuid::new_v4().to_string()),
            hangar_bay_id: Some(Uuid::new_v4().to_string()),
            loading_type: Some(vehicle::LoadingType::Land.into()),
            last_maintenance: Some(Utc::now().into()),
            next_maintenance: Some(Utc::now().into()),
            created_at: Some(Utc::now().into()),
            updated_at: Some(Utc::now().into()),
        };

        let vehicle = vehicle::Object {
            id: Uuid::new_v4().to_string(),
            data: Some(vehicle_data.clone()),
        };

        // valid
        let _ = Aircraft::try_from(vehicle.clone()).unwrap();

        // Invalid vehicle UUID
        let tmp = vehicle::Object {
            id: "invalid".to_string(),
            data: None,
        };
        let e = Aircraft::try_from(tmp).unwrap_err();
        assert_eq!(e, VehicleError::VehicleId);

        // Missing data
        let tmp = vehicle::Object {
            id: Uuid::new_v4().to_string(),
            data: None,
        };
        let e = Aircraft::try_from(tmp).unwrap_err();
        assert_eq!(e, VehicleError::Data);

        // Missing hangar_id
        let tmp = vehicle::Object {
            id: Uuid::new_v4().to_string(),
            data: Some(vehicle::Data {
                hangar_id: None,
                ..vehicle_data.clone()
            }),
        };
        let e = Aircraft::try_from(tmp).unwrap_err();
        assert_eq!(e, VehicleError::HangarId);

        // Missing hangar_bay_id
        let tmp = vehicle::Object {
            id: Uuid::new_v4().to_string(),
            data: Some(vehicle::Data {
                hangar_bay_id: None,
                ..vehicle_data.clone()
            }),
        };
        let e = Aircraft::try_from(tmp).unwrap_err();
        assert_eq!(e, VehicleError::HangarBayId);

        // Invalid hangar id
        let tmp = vehicle::Object {
            id: Uuid::new_v4().to_string(),
            data: Some(vehicle::Data {
                hangar_id: Some("invalid".to_string()),
                ..vehicle_data.clone()
            }),
        };
        let e = Aircraft::try_from(tmp).unwrap_err();
        assert_eq!(e, VehicleError::HangarId);

        // Invalid hangar bay id
        let tmp = vehicle::Object {
            id: Uuid::new_v4().to_string(),
            data: Some(vehicle::Data {
                hangar_bay_id: Some("invalid".to_string()),
                ..vehicle_data.clone()
            }),
        };
        let e = Aircraft::try_from(tmp).unwrap_err();
        assert_eq!(e, VehicleError::HangarBayId);

        // Missing schedule
        let tmp = vehicle::Object {
            id: Uuid::new_v4().to_string(),
            data: Some(vehicle::Data {
                schedule: None,
                ..vehicle_data.clone()
            }),
        };
        let e = Aircraft::try_from(tmp).unwrap_err();
        assert_eq!(e, VehicleError::NoSchedule);

        // Invalid schedule
        let tmp = vehicle::Object {
            id: Uuid::new_v4().to_string(),
            data: Some(vehicle::Data {
                schedule: Some("invalid".to_string()),
                ..vehicle_data.clone()
            }),
        };
        let e = Aircraft::try_from(tmp).unwrap_err();
        assert_eq!(e, VehicleError::Schedule);
    }
}
