use crate::grpc::client::GrpcClients;
use crate::router::flight_plan::*;
use crate::router::schedule::*;
use svc_storage_client_grpc::prelude::*;

use chrono::{DateTime, Duration, Utc};
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

/// Cannot schedule flight leaving within the next N minutes
pub const ADVANCE_NOTICE_MINUTES: i64 = 3;

/// Reasons for unavailable aircraft
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum VehicleError {
    /// Error from the vehicle client
    ClientError,

    /// Vehicle data is corrupt or invalid
    InvalidData,

    /// Vehicle doesn't have a schedule
    NoScheduleProvided,

    /// Vehicle has an invalid schedule
    InvalidSchedule,

    /// Internal error
    Internal,
}

impl std::fmt::Display for VehicleError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            VehicleError::ClientError => write!(f, "Vehicle client error"),
            VehicleError::InvalidData => write!(f, "Vehicle data is corrupt or invalid"),
            VehicleError::NoScheduleProvided => write!(f, "Vehicle doesn't have a schedule"),
            VehicleError::InvalidSchedule => write!(f, "Vehicle has an invalid schedule"),
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

        let flight_plan_timeslot = Timeslot {
            time_start: flight_plan.origin_timeslot_start,
            time_end: flight_plan.target_timeslot_start,
        };

        let timeslots = self.timeslot - flight_plan_timeslot;
        router_debug!(
            "(Availability::subtract) self: {:?}, fp: {:?}",
            self.timeslot,
            flight_plan_timeslot
        );
        router_debug!("(Availability::subtract) result: {:?}", timeslots);
        for timeslot in timeslots {
            let (vertiport_id, vertipad_id) =
                if timeslot.time_start < flight_plan_timeslot.time_start {
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
        let vehicle_uuid = match Uuid::parse_str(&vehicle.id) {
            Ok(uuid) => uuid.to_string(),
            Err(e) => {
                router_error!("(try_from) Vehicle {} has invalid UUID: {}", vehicle.id, e);

                return Err(VehicleError::InvalidData);
            }
        };

        let Some(data) = vehicle.data else {
            router_error!("(try_from) Vehicle doesn't have data: {:?}", vehicle);

            return Err(VehicleError::InvalidData);
        };

        let Some(hangar_id) = data.hangar_id else {
            router_error!(
                "(try_from) Vehicle {} doesn't have hangar_id.",
                vehicle_uuid
            );

            return Err(VehicleError::InvalidData);
        };

        let hangar_id = match Uuid::parse_str(&hangar_id) {
            Ok(uuid) => uuid.to_string(),
            Err(e) => {
                router_error!(
                    "(try_from) Vehicle {} has invalid hangar_id: {}",
                    vehicle_uuid,
                    e
                );

                return Err(VehicleError::InvalidData);
            }
        };

        let Some(hangar_bay_id) = data.hangar_bay_id else {
            router_error!(
                "(try_from) Vehicle {} doesn't have hangar_bay_id.",
                vehicle_uuid
            );

            return Err(VehicleError::InvalidData);
        };

        let hangar_bay_id = match Uuid::parse_str(&hangar_bay_id) {
            Ok(uuid) => uuid.to_string(),
            Err(e) => {
                router_error!(
                    "(try_from) Vehicle {} has invalid hangar_bay_id: {}",
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
            hangar_id,
            hangar_bay_id,
        })
    }
}

/// Request a list of all aircraft from svc-storage
pub async fn get_aircraft(
    clients: &GrpcClients,
    aircraft_id: Option<String>,
) -> Result<Vec<Aircraft>, VehicleError> {
    // TODO(R4): Private aircraft, disabled aircraft, etc. should be filtered out here
    //  This is a lot of aircraft. Possible filters:
    //   geographical area within N kilometers of request origin vertiport
    //   private aircraft
    //   disabled aircraft

    // TODO(R4): Ignore aircraft that haven't been updated recently
    // We should further limit this, but for now we'll just get all aircraft
    //  Need something to sort by, ascending distance from the
    //  departure vertiport or charge level before cutting off the list

    let mut filter = match aircraft_id {
        Some(id) => AdvancedSearchFilter::search_equals("vehicle_id".to_string(), id.to_string()),
        None => AdvancedSearchFilter::default(),
    };

    filter.results_per_page = 1000;

    let Ok(response) = clients.storage.vehicle.search(filter).await else {
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
pub fn estimate_flight_time_seconds(distance_meters: &f32) -> Result<Duration, VehicleError> {
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

            let total_duration_s: f32 = liftoff_duration_s + cruise_duration_s + landing_duration_s;
            Duration::try_milliseconds((total_duration_s * 1000.0) as i64).ok_or_else(|| {
                router_error!("(estimate_flight_time_seconds) error creating time delta.");
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
    router_debug!("(get_aircraft_availabilities) aircraft: {:?}", aircraft);
    let deadhead_padding: Duration = Duration::try_hours(2).ok_or_else(|| {
        router_error!("(get_aircraft_availabilities) error creating time delta.");
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
                &(timeslot.time_start - deadhead_padding),
                &(timeslot.time_end + deadhead_padding),
            )
            .map_err(|e| {
                router_error!(
                    "(get_aircraft_availabilities) error creating timeslots: {}",
                    e
                );
                VehicleError::Internal
            })?
            .into_iter()
            .for_each(|mut timeslot| {
                // ignore timeslots that are in the past
                // the user will need time to select an itinerary, by the time they select
                // the itinerary start should not be in the past. Add delta to compensate
                timeslot.time_start = timeslot.time_start.max(*earliest_departure_time);
                if timeslot.time_start >= timeslot.time_end {
                    return;
                }

                aircraft_availabilities
                    .entry(a.vehicle_uuid.clone())
                    .or_default()
                    .push(Availability {
                        timeslot,
                        vertiport_id: hangar_id.clone(),
                        vertipad_id: hangar_bay_id.clone(),
                    });
            });
    }

    router_debug!(
        "(get_aircraft_availabilities) aircraft base availabilities: {:?}",
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
            router_warn!(
                "(get_aircraft_availabilities) Flight plan for unknown aircraft: {}",
                fp.vehicle_id
            );
        }
    });

    router_debug!(
        "(get_aircraft_availabilities) aircraft availabilities after flight plans: {:?}",
        aircraft_availabilities
    );

    Ok(aircraft_availabilities)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone, Utc};

    #[test]
    fn test_subtract_flight_plan() {
        let vertiport_start_id = Uuid::new_v4().to_string();
        let vertipad_start_id = Uuid::new_v4().to_string();
        let vertiport_middle_id = Uuid::new_v4().to_string();
        let vertipad_middle_id = Uuid::new_v4().to_string();
        let aircraft_id = Uuid::new_v4().to_string();

        let year = Utc::now().year() + 1;
        let chrono::LocalResult::Single(dt_start) = Utc.with_ymd_and_hms(year, 10, 20, 0, 0, 0)
        else {
            panic!();
        };

        let availability = Availability {
            timeslot: Timeslot {
                time_start: dt_start,
                time_end: dt_start + Duration::try_hours(2).unwrap(),
            },
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
            },
        ];

        let result = availability.subtract(&flight_plans[0]);
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            Availability {
                timeslot: Timeslot {
                    time_start: dt_start,
                    time_end: flight_plans[0].origin_timeslot_start,
                },
                vertiport_id: vertiport_start_id.clone(),
                vertipad_id: vertipad_start_id.clone()
            }
        );
        assert_eq!(
            result[1],
            Availability {
                timeslot: Timeslot {
                    time_start: flight_plans[0].target_timeslot_start,
                    time_end: dt_start + Duration::try_hours(2).unwrap(),
                },
                vertiport_id: vertiport_middle_id.clone(),
                vertipad_id: vertipad_middle_id.clone()
            }
        );

        let result = availability.subtract(&flight_plans[1]);
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            Availability {
                timeslot: Timeslot {
                    time_start: dt_start,
                    time_end: flight_plans[1].origin_timeslot_start,
                },
                vertiport_id: vertiport_start_id.clone(),
                vertipad_id: vertipad_start_id.clone()
            }
        );
        assert_eq!(
            result[1],
            Availability {
                timeslot: Timeslot {
                    time_start: flight_plans[1].target_timeslot_start,
                    time_end: dt_start + Duration::try_hours(2).unwrap(),
                },
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
                timeslot: Timeslot {
                    time_start: dt_start,
                    time_end: flight_plans[0].origin_timeslot_start,
                },
                vertiport_id: vertiport_start_id.clone(),
                vertipad_id: vertipad_start_id.clone()
            }
        );
        assert_eq!(
            availabilities[1],
            Availability {
                timeslot: Timeslot {
                    time_start: flight_plans[0].target_timeslot_start,
                    time_end: flight_plans[1].origin_timeslot_start,
                },
                vertiport_id: vertiport_middle_id.clone(),
                vertipad_id: vertipad_middle_id.clone()
            }
        );
        assert_eq!(
            availabilities[2],
            Availability {
                timeslot: Timeslot {
                    time_start: flight_plans[1].target_timeslot_start,
                    time_end: dt_start + Duration::try_hours(2).unwrap(),
                },
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
        let chrono::LocalResult::Single(dt_start) = Utc.with_ymd_and_hms(year, 10, 20, 0, 0, 0)
        else {
            panic!();
        };

        let timeslots = schedule
            .clone()
            .to_timeslots(&dt_start, &(dt_start + Duration::try_hours(2).unwrap()))
            .unwrap();
        assert_eq!(timeslots.len(), 1);
        assert_eq!(
            timeslots[0],
            Timeslot {
                time_start: dt_start,
                time_end: dt_start + Duration::try_hours(2).unwrap(),
            }
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

        let timeslot = Timeslot {
            time_start: dt_start,
            time_end: dt_start + Duration::try_hours(2).unwrap(),
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
            },
        ];

        let mut gaps =
            get_aircraft_availabilities(&flight_plans, &timeslot.time_start, &aircraft, &timeslot)
                .unwrap();

        println!("gaps: {:?}", gaps);

        assert_eq!(gaps.len(), 1);
        let gaps = gaps.get_mut(&aircraft_id).unwrap();

        println!("gaps: {:?}", gaps);

        assert_eq!(gaps.len(), 3);
        gaps.sort_by(|a, b| b.timeslot.time_start.cmp(&a.timeslot.time_start));
        assert_eq!(
            gaps.pop().unwrap(),
            Availability {
                timeslot: Timeslot {
                    time_start: dt_start,
                    time_end: flight_plans[0].origin_timeslot_start
                },
                vertiport_id: vertiport_start_id.clone(),
                vertipad_id: vertipad_start_id.clone()
            }
        );

        assert_eq!(
            gaps.pop().unwrap(),
            Availability {
                timeslot: Timeslot {
                    time_start: flight_plans[0].target_timeslot_start,
                    time_end: flight_plans[1].origin_timeslot_start
                },
                vertiport_id: vertiport_middle_id.clone(),
                vertipad_id: vertipad_middle_id.clone()
            }
        );

        assert_eq!(
            gaps.pop().unwrap(),
            Availability {
                timeslot: Timeslot {
                    time_start: flight_plans[1].target_timeslot_start,
                    // see 'deadhead_padding' in the function
                    // the vehicle schedule in this example is 3 hours long, less than the deadhead padding,
                    //  so the end time is the end of the vehicle schedule in this case
                    time_end: dt_start + Duration::try_hours(vehicle_duration_hours).unwrap(), // the vehicle schedule is 3 hours long
                },
                vertiport_id: vertiport_start_id,
                vertipad_id: vertipad_start_id
            }
        );
    }
}
