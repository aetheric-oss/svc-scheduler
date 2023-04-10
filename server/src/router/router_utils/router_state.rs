//! Stores the state of the router
use crate::router::router_types::{
    location::Location,
    node::Node,
    router::engine::{Algorithm, Router},
    status,
};
use crate::router::router_utils::{generator::generate_nodes_near, haversine, schedule::Calendar};
use chrono::{DateTime, Duration, NaiveDateTime, TimeZone};
use once_cell::sync::OnceCell;
use ordered_float::OrderedFloat;
use prost_types::Timestamp;
use rrule::Tz;
use std::collections::HashMap;
use std::str::FromStr;

// Expose so svc-scheduler doesn't assume same svc-storage version
pub use svc_storage_client_grpc::resources::flight_plan::{
    Data as FlightPlanData, Object as FlightPlan,
};
pub use svc_storage_client_grpc::resources::vehicle::Object as Vehicle;
pub use svc_storage_client_grpc::resources::vertipad::Object as Vertipad;
pub use svc_storage_client_grpc::resources::vertiport::Object as Vertiport;

/// Query struct for generating nodes near a location.
#[derive(Debug, Copy, Clone)]
pub struct NearbyLocationQuery {
    ///location
    pub location: Location,
    ///radius
    pub radius: f32,
    ///capacity
    pub capacity: i32,
}

/// Query struct to find a route between two nodes
#[derive(Debug, Copy, Clone)]
pub struct RouteQuery {
    ///aircraft
    pub aircraft: Aircraft,
    ///from
    pub from: &'static Node,
    ///to
    pub to: &'static Node,
}

/// Enum with all Aircraft types
#[derive(Debug, Copy, Clone)]
pub enum Aircraft {
    ///Cargo aircraft
    Cargo,
}
/// List of vertiport nodes for routing
pub static NODES: OnceCell<Vec<Node>> = OnceCell::new();
/// Cargo router
pub static ARROW_CARGO_ROUTER: OnceCell<Router> = OnceCell::new();

static ARROW_CARGO_CONSTRAINT: f32 = 75.0;

#[allow(dead_code)]
/// SF central location
pub static SAN_FRANCISCO: Location = Location {
    latitude: OrderedFloat(37.7749),
    longitude: OrderedFloat(-122.4194),
    altitude_meters: OrderedFloat(0.0),
};

/// Time to block vertiport for cargo loading and takeoff
pub const LOADING_AND_TAKEOFF_TIME_MIN: f32 = 10.0;
/// Time to block vertiport for cargo unloading and landing
pub const LANDING_AND_UNLOADING_TIME_MIN: f32 = 10.0;
/// Average speed of cargo aircraft
pub const AVG_SPEED_KMH: f32 = 60.0;
/// Minimum time between suggested flight plans in case of multiple flights available
pub const FLIGHT_PLAN_GAP_MINUTES: f32 = 5.0;
/// Max amount of flight plans to return in case of large time window and multiple flights available
pub const MAX_RETURNED_FLIGHT_PLANS: i64 = 10;

/// Helper function to check if two time ranges overlap (touching ranges are not considered overlapping)
/// All parameters are in seconds since epoch
fn time_ranges_overlap(start1: i64, end1: i64, start2: i64, end2: i64) -> bool {
    start1 < end2 && start2 < end1
}

/// Helper function to create a flight plan data object from 5 required parameters
fn create_flight_plan_data(
    vehicle_id: String,
    departure_vertiport_id: String,
    arrival_vertiport_id: String,
    departure_time: DateTime<Tz>,
    arrival_time: DateTime<Tz>,
) -> FlightPlanData {
    FlightPlanData {
        pilot_id: "".to_string(),
        vehicle_id,
        cargo_weight_grams: vec![],
        weather_conditions: None,
        departure_vertiport_id: Some(departure_vertiport_id),
        destination_vertiport_id: Some(arrival_vertiport_id),
        scheduled_departure: Some(Timestamp {
            seconds: departure_time.timestamp(),
            nanos: departure_time.timestamp_subsec_nanos() as i32,
        }),
        scheduled_arrival: Some(Timestamp {
            seconds: arrival_time.timestamp(),
            nanos: arrival_time.timestamp_subsec_nanos() as i32,
        }),
        actual_departure: None,
        actual_arrival: None,
        flight_release_approval: None,
        flight_plan_submitted: None,
        approved_by: None,
        flight_status: 0,
        flight_priority: 0,
        departure_vertipad_id: "".to_string(),
        destination_vertipad_id: "".to_string(),
        flight_distance_meters: 0,
    }
}

/// Checks if a vehicle is available for a given time window date_from to
///    date_from + flight_duration_minutes (this includes takeoff and landing time)
/// This checks both static schedule of the aircraft and existing flight plans which might overlap.
pub fn is_vehicle_available(
    vehicle: &Vehicle,
    date_from: DateTime<Tz>,
    flight_duration_minutes: i64,
    existing_flight_plans: &[FlightPlan],
) -> Result<bool, String> {
    let vehicle_data = vehicle.data.as_ref().unwrap();

    // TODO R3: What's the default if a schedule isn't provided?
    let Some(vehicle_schedule) = vehicle_data.schedule.as_ref() else {
        return Ok(true);
    };

    let vehicle_schedule = vehicle_schedule.as_str();
    let Ok(vehicle_schedule) = Calendar::from_str(vehicle_schedule) else {
        router_debug!(
            "Invalid schedule for vehicle {}: {}",
            vehicle.id,
            vehicle_schedule
        );

        return Err(
            "Invalid schedule for vehicle.".to_string(),
        );
    };

    let date_to = date_from + Duration::minutes(flight_duration_minutes);
    //check if vehicle is available as per schedule
    if !vehicle_schedule.is_available_between(date_from, date_to) {
        return Ok(false);
    }

    //check if vehicle is available as per existing flight plans
    let conflicting_flight_plans_count = existing_flight_plans
        .iter()
        .filter(|flight_plan| {
            flight_plan.data.as_ref().unwrap().vehicle_id == vehicle.id
                && time_ranges_overlap(
                    flight_plan
                        .data
                        .as_ref()
                        .unwrap()
                        .scheduled_departure
                        .as_ref()
                        .unwrap()
                        .seconds,
                    flight_plan
                        .data
                        .as_ref()
                        .unwrap()
                        .scheduled_arrival
                        .as_ref()
                        .unwrap()
                        .seconds,
                    date_from.timestamp(),
                    date_to.timestamp(),
                )
        })
        .count();
    if conflicting_flight_plans_count > 0 {
        return Ok(false);
    }

    Ok(true)
}

/// Checks if vertiport is available for a given time window from date_from to date_from + duration
/// of how long vertiport is blocked by takeoff/landing
/// This checks both static schedule of vertiport and existing flight plans which might overlap.
/// is_departure_vertiport is used to determine if we are checking for departure or arrival vertiport
pub fn is_vertiport_available(
    vertiport_id: String,
    vertiport_schedule: Option<String>,
    vertipads: &[Vertipad],
    date_from: DateTime<Tz>,
    existing_flight_plans: &[FlightPlan],
    is_departure_vertiport: bool,
) -> (bool, Vec<(String, i64)>) {
    let mut num_vertipads = vertipads.len();
    if num_vertipads == 0 {
        num_vertipads = 1
    };
    let vertiport_schedule =
        Calendar::from_str(vertiport_schedule.as_ref().unwrap().as_str()).unwrap();
    let block_vertiport_minutes: i64 = if is_departure_vertiport {
        LOADING_AND_TAKEOFF_TIME_MIN as i64
    } else {
        LANDING_AND_UNLOADING_TIME_MIN as i64
    };
    let date_to = date_from + Duration::minutes(block_vertiport_minutes);
    //check if vertiport is available as per schedule
    if !vertiport_schedule.is_available_between(date_from, date_to) {
        return (false, vec![]);
    }
    let conflicting_flight_plans_count = existing_flight_plans
        .iter()
        .filter(|flight_plan| {
            if is_departure_vertiport {
                flight_plan
                    .data
                    .as_ref()
                    .unwrap()
                    .departure_vertiport_id
                    .clone()
                    .unwrap()
                    == vertiport_id
                    && flight_plan
                        .data
                        .as_ref()
                        .unwrap()
                        .scheduled_departure
                        .as_ref()
                        .unwrap()
                        .seconds
                        > date_from.timestamp() - block_vertiport_minutes * 60
                    && flight_plan
                        .data
                        .as_ref()
                        .unwrap()
                        .scheduled_departure
                        .as_ref()
                        .unwrap()
                        .seconds
                        < date_to.timestamp() + block_vertiport_minutes * 60
            } else {
                flight_plan
                    .data
                    .as_ref()
                    .unwrap()
                    .destination_vertiport_id
                    .clone()
                    .unwrap()
                    == vertiport_id
                    && flight_plan
                        .data
                        .as_ref()
                        .unwrap()
                        .scheduled_arrival
                        .as_ref()
                        .unwrap()
                        .seconds
                        > date_from.timestamp() - block_vertiport_minutes * 60
                    && flight_plan
                        .data
                        .as_ref()
                        .unwrap()
                        .scheduled_arrival
                        .as_ref()
                        .unwrap()
                        .seconds
                        < date_to.timestamp() + block_vertiport_minutes * 60
            }
        })
        .count();
    let res = if num_vertipads > 1 {
        let vehicles_at_vertiport =
            get_all_vehicles_scheduled_for_vertiport(&vertiport_id, date_to, existing_flight_plans);
        (
            vehicles_at_vertiport.len() < num_vertipads,
            vehicles_at_vertiport,
        )
    } else {
        (conflicting_flight_plans_count == 0, vec![])
    };
    router_debug!(
        "Checking {} is departure: {}, is available for {} - {}? {}",
        vertiport_id,
        is_departure_vertiport,
        date_from,
        date_to,
        res.0,
    );
    res
}

///Finds all vehicles which are parked at or in flight to the vertiport at specific timestamp
/// Returns vector of tuples of (vehicle_id, minutes_to_arrival) where minutes_to_arrival is 0 if vehicle is parked at the vertiport
/// and up to 10 minutes if vehicle is landing
pub fn get_all_vehicles_scheduled_for_vertiport(
    vertiport_id: &str,
    timestamp: DateTime<Tz>,
    existing_flight_plans: &[FlightPlan],
) -> Vec<(String, i64)> {
    let mut vehicles_plans_sorted: HashMap<String, Vec<FlightPlan>> = HashMap::new();
    existing_flight_plans
        .iter()
        .filter(|flight_plan| {
            flight_plan
                .data
                .as_ref()
                .unwrap()
                .destination_vertiport_id
                .as_ref()
                .unwrap()
                == vertiport_id
                && flight_plan
                    .data
                    .as_ref()
                    .unwrap()
                    .scheduled_arrival
                    .as_ref()
                    .unwrap()
                    .seconds // arrival time needs to be less than 2x time needed - to allow landing and and then take off again)
                    < timestamp.timestamp() + LANDING_AND_UNLOADING_TIME_MIN as i64 * 60
        })
        .for_each(|flight_plan| {
            let vehicle_id = flight_plan.data.as_ref().unwrap().vehicle_id.clone();
            let entry = vehicles_plans_sorted.entry(vehicle_id).or_default();
            entry.push(flight_plan.clone());
        });
    //sort by scheduled arrival, latest first
    vehicles_plans_sorted
        .iter_mut()
        .for_each(|(_, flight_plans)| {
            flight_plans.sort_by(|a, b| {
                b.data
                    .as_ref()
                    .unwrap()
                    .scheduled_arrival
                    .as_ref()
                    .unwrap()
                    .seconds
                    .cmp(
                        &a.data
                            .as_ref()
                            .unwrap()
                            .scheduled_arrival
                            .as_ref()
                            .unwrap()
                            .seconds,
                    )
            });
        });
    //return only the latest flight plan for each vehicle
    let vehicles = vehicles_plans_sorted
        .iter()
        .map(|(vehicle_id, flight_plans)| {
            let mut minutes_to_arrival = (flight_plans
                .first()
                .unwrap()
                .data
                .as_ref()
                .unwrap()
                .scheduled_arrival
                .as_ref()
                .unwrap()
                .seconds
                - timestamp.timestamp())
                / 60;
            if minutes_to_arrival < 0 {
                minutes_to_arrival = 0;
            }
            (vehicle_id.clone(), minutes_to_arrival)
        })
        .collect();
    router_debug!(
        "Vehicles at vertiport: {} at a time: {} : {:?}",
        &vertiport_id,
        timestamp,
        vehicles
    );
    vehicles
}

/// Gets vehicle location (vertiport_id) at given timestamp
/// Returns tuple of (vertiport_id, minutes_to_arrival)
/// If minutes_to_arrival is 0, vehicle is parked at the vertiport,
/// otherwise it is in flight to the vertiport and should arrive in minutes_to_arrival
pub fn get_vehicle_scheduled_location(
    vehicle: &Vehicle,
    timestamp: DateTime<Tz>,
    existing_flight_plans: &[FlightPlan],
) -> (String, i64) {
    let mut vehicle_flight_plans = existing_flight_plans
        .iter()
        .filter(|flight_plan| {
            flight_plan.data.as_ref().unwrap().vehicle_id == vehicle.id
                && flight_plan
                    .data
                    .as_ref()
                    .unwrap()
                    .scheduled_departure
                    .as_ref()
                    .unwrap()
                    .seconds
                    <= timestamp.timestamp()
        })
        .collect::<Vec<&FlightPlan>>();
    vehicle_flight_plans.sort_by(|a, b| {
        b.data
            .as_ref()
            .unwrap()
            .scheduled_departure
            .as_ref()
            .unwrap()
            .seconds
            .cmp(
                &a.data
                    .as_ref()
                    .unwrap()
                    .scheduled_departure
                    .as_ref()
                    .unwrap()
                    .seconds,
            )
    });
    if vehicle_flight_plans.is_empty() {
        return (
            vehicle
                .data
                .as_ref()
                .unwrap()
                .last_vertiport_id
                .as_ref()
                .unwrap()
                .clone(),
            0,
        );
    }
    let vehicle_flight_plan = vehicle_flight_plans.first().unwrap();
    router_debug!(
        "Vehicle {} had last flight plan {} with destination {}",
        vehicle.id,
        vehicle_flight_plan.id.clone(),
        vehicle_flight_plan
            .data
            .as_ref()
            .unwrap()
            .destination_vertiport_id
            .as_ref()
            .unwrap()
    );
    let mut minutes_to_arrival = (vehicle_flight_plan
        .data
        .as_ref()
        .unwrap()
        .scheduled_arrival
        .as_ref()
        .unwrap()
        .seconds
        - timestamp.timestamp())
        / 60;
    if minutes_to_arrival < 0 {
        minutes_to_arrival = 0;
    }
    (
        vehicle_flight_plan
            .data
            .as_ref()
            .unwrap()
            .destination_vertiport_id
            .as_ref()
            .unwrap()
            .to_string(),
        minutes_to_arrival,
    )
}

/// Gets flight durations from all vertiports in current router to the requested vertiport
/// All distances between vertiports are calculated during the router initialization (costs of edges)
/// so this function only filters the edges and calculates flight duration based on the distance
pub fn get_all_flight_durations_to_vertiport(vertiport_id: &str) -> HashMap<&Node, i64> {
    let mut durations = HashMap::new();
    ARROW_CARGO_ROUTER
        .get()
        .unwrap()
        .edges
        .iter()
        .for_each(|edge| {
            if edge.to.uid == vertiport_id {
                durations.insert(
                    edge.from,
                    estimate_flight_time_minutes(f32::from(edge.cost), Aircraft::Cargo) as i64,
                );
            }
        });
    durations
}

/// Gets nearest gap for a reroute flight - takeoff and landing at the same vertiport
fn find_nearest_gap_for_reroute_flight(
    vertiport_id: String,
    vertiport_schedule: Option<String>,
    vertipads: &[Vertipad],
    date_from: DateTime<Tz>,
    vehicle_id: String,
    existing_flight_plans: &[FlightPlan],
) -> Option<DateTime<Tz>> {
    let mut time_from: Option<DateTime<Tz>> = None;
    for i in 0..6 {
        let added_time = date_from + Duration::minutes(i * LOADING_AND_TAKEOFF_TIME_MIN as i64);
        let (dep, vehicles_dep) = is_vertiport_available(
            vertiport_id.clone(),
            vertiport_schedule.clone(),
            vertipads,
            added_time,
            existing_flight_plans,
            true,
        );
        let (arr, vehicles_arr) = is_vertiport_available(
            vertiport_id.clone(),
            vertiport_schedule.clone(),
            vertipads,
            added_time + Duration::minutes(LANDING_AND_UNLOADING_TIME_MIN as i64),
            existing_flight_plans,
            false,
        );
        if (dep || vehicles_dep.contains(&(vehicle_id.clone(), 0)))
            && (arr || vehicles_arr.contains(&(vehicle_id.clone(), 0)))
        {
            time_from = Some(added_time);
            break;
        }
    }
    time_from
}

/// For the scenario where there is no available vehicle for the flight plan, this function find a deadhead flight plan
/// - summoning vehicle from the nearest vertiport to the departure vertiport so it can depart on time
/// Returns available vehicle and deadhead flight plan data if found, or (None, None) otherwise
#[allow(clippy::too_many_arguments)]
pub fn find_deadhead_flight_plan(
    nearest_vertiports_from_departure: &Vec<&Node>,
    departure_vertiport_durations: &HashMap<&Node, i64>,
    vehicles: &Vec<Vehicle>,
    vertiport_depart: &Vertiport,
    vertipads_depart: &[Vertipad],
    departure_time: DateTime<Tz>,
    existing_flight_plans: &[FlightPlan],
    block_aircraft_and_vertiports_minutes: i64,
) -> (Option<Vehicle>, Option<FlightPlanData>) {
    for &vertiport in nearest_vertiports_from_departure {
        let n_duration = *departure_vertiport_durations.get(vertiport).unwrap();
        for vehicle in vehicles {
            router_debug!(
                "DH: Checking vehicle id:{} for departure time: {}",
                &vehicle.id,
                departure_time
            );
            let (vehicle_dest_vertiport, _minutes_to_arrival) = get_vehicle_scheduled_location(
                vehicle,
                departure_time - Duration::minutes(n_duration),
                existing_flight_plans,
            );
            if vehicle_dest_vertiport != *vertiport.uid {
                router_debug!(
                    "DH: Vehicle id:{} not at or arriving to vertiport id:{}",
                    &vehicle.id,
                    vehicle_dest_vertiport
                );
                continue;
            }

            let result = is_vehicle_available(
                vehicle,
                departure_time - Duration::minutes(n_duration),
                block_aircraft_and_vertiports_minutes,
                existing_flight_plans,
            );

            let Ok(is_vehicle_available) = result else {
                router_debug!(
                    "Unable to determine vehicle availability: (id {}) {}",
                    &vehicle.id, result.err().unwrap()
                );
                continue;
            };

            if !is_vehicle_available {
                router_debug!(
                            "DH: Vehicle id:{} not available for departure time: {} and duration {} minutes",
                            &vehicle.id, departure_time - Duration::minutes(n_duration), block_aircraft_and_vertiports_minutes
                        );
                continue;
            }
            let (is_departure_vertiport_available, _) = is_vertiport_available(
                vertiport.uid.clone(),
                vertiport.schedule.clone(),
                &[],
                departure_time - Duration::minutes(n_duration),
                existing_flight_plans,
                true,
            );
            let (is_arrival_vertiport_available, _) = is_vertiport_available(
                vertiport_depart.id.clone(),
                vertiport_depart.data.as_ref().unwrap().schedule.clone(),
                vertipads_depart,
                departure_time - Duration::minutes(LANDING_AND_UNLOADING_TIME_MIN as i64),
                existing_flight_plans,
                false,
            );
            router_debug!(
                "DH: DEPARTURE TIME: {}, {}, {}",
                departure_time,
                is_departure_vertiport_available,
                is_arrival_vertiport_available
            );
            if !is_departure_vertiport_available {
                router_debug!(
                    "DH: Departure vertiport not available for departure time {}",
                    departure_time - Duration::minutes(n_duration)
                );
                continue;
            }
            if !is_arrival_vertiport_available {
                router_debug!(
                    "DH: Arrival vertiport not available for departure time {}",
                    departure_time - Duration::minutes(LANDING_AND_UNLOADING_TIME_MIN as i64)
                );
                continue;
            }
            // add deadhead flight plan and return
            router_debug!(
                        "DH: Found available vehicle with id: {} from vertiport id: {}, for a DH flight for a departure time {}", vehicle.id, vertiport.uid.clone(),
                        departure_time - Duration::minutes(n_duration)
                    );
            return (
                Some(vehicle.clone()),
                Some(create_flight_plan_data(
                    vehicle.id.clone(),
                    vertiport.uid.clone(),
                    vertiport_depart.id.clone(),
                    departure_time - Duration::minutes(n_duration),
                    departure_time,
                )),
            );
        }
    }
    (None, None)
}

/// In the scenario there is no vehicle available at the arrival vertiport, we can check
/// if there is availability at some other vertiport and re-route idle vehicle there.
/// This function finds such a flight plan and returns it
pub fn find_rerouted_vehicle_flight_plan(
    vehicles_at_arrival_airport: &[(String, i64)],
    vertiport_arrive: &Vertiport,
    vertipads_arrive: &[Vertipad],
    arrival_time: &DateTime<Tz>,
    existing_flight_plans: &[FlightPlan],
) -> Option<FlightPlanData> {
    let found_vehicle = vehicles_at_arrival_airport
        .iter() //if there is a parked vehicle at the arrival vertiport, we can move it to some other vertiport
        .find(|(_, minutes_to_arrival)| *minutes_to_arrival == 0);
    found_vehicle?;
    router_debug!("Checking if idle vehicle from the arrival airport can be re-routed");
    //todo this should re-route the vehicle to the nearest vertiport or HUB, but
    // we don't have vertipads or HUB id in the graph to do this.
    // So we are just re-routing to the same vertiport in the future time instead
    let found_gap = find_nearest_gap_for_reroute_flight(
        vertiport_arrive.id.clone(),
        vertiport_arrive.data.as_ref().unwrap().schedule.clone(),
        vertipads_arrive,
        *arrival_time,
        found_vehicle.unwrap().0.clone(),
        existing_flight_plans,
    );
    found_gap?;
    router_debug!(
        "Found a gap for re-routing idle vehicle from the arrival vertiport {}",
        found_gap.unwrap()
    );
    Some(create_flight_plan_data(
        found_vehicle.unwrap().0.clone(),
        vertiport_arrive.id.clone(),
        vertiport_arrive.id.clone(),
        found_gap.unwrap(),
        found_gap.unwrap()
            + Duration::minutes(
                LANDING_AND_UNLOADING_TIME_MIN as i64 + LOADING_AND_TAKEOFF_TIME_MIN as i64,
            ),
    ))
}

/// Gets nearest vertiports to the requested vertiport
/// Returns tuple of:
///    sorted_vertiports_by_durations - vector of &Nodes,
///    vertiport_durations - hashmap of &Node and flight duration in minutes)
pub fn get_nearest_vertiports_vertiport_id(
    vertiport_depart: &Vertiport,
) -> (Vec<&Node>, HashMap<&Node, i64>) {
    let vertiport_durations = get_all_flight_durations_to_vertiport(&vertiport_depart.id);
    let mut vd_vec = Vec::from_iter(vertiport_durations.iter());
    vd_vec.sort_by(|a, b| a.1.cmp(b.1));
    let sorted_vertiports_by_durations = vd_vec.iter().map(|(a, _b)| **a).collect::<Vec<&Node>>();
    router_debug!("Vertiport durations: {:?}", &vertiport_durations);
    router_debug!("Sorted vertiports: {:?}", &sorted_vertiports_by_durations);
    (sorted_vertiports_by_durations, vertiport_durations)
}

/// Creates all possible flight plans based on the given request
/// * `vertiport_depart` - Departure vertiport - svc-storage format
/// * `vertiport_arrive` - Arrival vertiport - svc-storage format
/// * `earliest_departure_time` - Earliest departure time of the time window
/// * `latest_arrival_time` - Latest arrival time of the time window
/// * `aircrafts` - Aircrafts serving the route and vertiports
/// # Returns
/// A vector of flight plans
#[allow(clippy::too_many_arguments)]
pub fn get_possible_flights(
    vertiport_depart: Vertiport,
    vertiport_arrive: Vertiport,
    vertipads_depart: Vec<Vertipad>,
    vertipads_arrive: Vec<Vertipad>,
    earliest_departure_time: Option<Timestamp>,
    latest_arrival_time: Option<Timestamp>,
    vehicles: Vec<Vehicle>,
    existing_flight_plans: Vec<FlightPlan>,
) -> Result<Vec<(FlightPlanData, Vec<FlightPlanData>)>, String> {
    router_info!("Finding possible flights");
    if earliest_departure_time.is_none() || latest_arrival_time.is_none() {
        router_error!("Both earliest departure and latest arrival time must be specified");
        return Err(
            "Both earliest departure and latest arrival time must be specified".to_string(),
        );
    }
    //1. Find route and cost between requested vertiports
    router_info!("[1/5]: Finding route between vertiports");
    if !is_router_initialized() {
        router_error!("Router not initialized");
        return Err("Router not initialized".to_string());
    }
    let (route, cost) = get_route(RouteQuery {
        from: get_node_by_id(&vertiport_depart.id)?,
        to: get_node_by_id(&vertiport_arrive.id)?,
        aircraft: Aircraft::Cargo,
    })?;
    router_debug!("Route: {:?}", route);
    router_debug!("Cost: {:?}", cost);
    if route.is_empty() {
        router_error!("No route found");
        return Err("Route between vertiports not found".to_string());
    }
    //1.1 Create a sorted vector of vertiports nearest to the departure and arrival vertiport (in case we need to create a deadhead flight)
    let (nearest_vertiports_from_departure, departure_vertiport_durations) =
        get_nearest_vertiports_vertiport_id(&vertiport_depart);

    //2. calculate blocking times for each vertiport and aircraft
    router_info!("[2/5]: Calculating blocking times");

    let block_aircraft_and_vertiports_minutes = estimate_flight_time_minutes(cost, Aircraft::Cargo);

    router_debug!(
        "Estimated flight time in minutes including takeoff and landing: {}",
        block_aircraft_and_vertiports_minutes
    );

    let time_window_duration_minutes: f32 = ((latest_arrival_time.as_ref().unwrap().seconds
        - earliest_departure_time.as_ref().unwrap().seconds)
        / 60) as f32;
    router_debug!(
        "Time window duration in minutes: {}",
        time_window_duration_minutes
    );
    if (time_window_duration_minutes - block_aircraft_and_vertiports_minutes) < 0.0 {
        router_error!("Time window too small to schedule flight");
        return Err("Time window too small to schedule flight".to_string());
    }
    let mut num_flight_options: i64 = ((time_window_duration_minutes
        - block_aircraft_and_vertiports_minutes)
        / FLIGHT_PLAN_GAP_MINUTES)
        .floor() as i64
        + 1;
    if num_flight_options > MAX_RETURNED_FLIGHT_PLANS {
        num_flight_options = MAX_RETURNED_FLIGHT_PLANS;
    }
    //3. check vertiport schedules and flight plans
    router_info!(
        "[3/5]: Checking vertiport schedules and flight plans for {} possible flight plans",
        num_flight_options
    );
    let mut flight_plans: Vec<(FlightPlanData, Vec<FlightPlanData>)> = vec![];
    for i in 0..num_flight_options {
        let mut deadhead_flights: Vec<FlightPlanData> = vec![];
        let departure_time = Tz::UTC.from_utc_datetime(
            &NaiveDateTime::from_timestamp_opt(
                earliest_departure_time.as_ref().unwrap().seconds
                    + i * 60 * FLIGHT_PLAN_GAP_MINUTES as i64,
                earliest_departure_time.as_ref().unwrap().nanos as u32,
            )
            .ok_or("Invalid departure_time")?,
        );
        let arrival_time =
            departure_time + Duration::minutes(block_aircraft_and_vertiports_minutes as i64);
        let (is_departure_vertiport_available, _) = is_vertiport_available(
            vertiport_depart.id.clone(),
            vertiport_depart.data.as_ref().unwrap().schedule.clone(),
            &vertipads_depart,
            departure_time,
            &existing_flight_plans,
            true,
        );
        let (is_arrival_vertiport_available, vehicles_at_arrival_airport) = is_vertiport_available(
            vertiport_arrive.id.clone(),
            vertiport_arrive.data.as_ref().unwrap().schedule.clone(),
            &vertipads_arrive,
            arrival_time - Duration::minutes(LANDING_AND_UNLOADING_TIME_MIN as i64),
            &existing_flight_plans,
            false,
        );
        router_debug!(
            "DEPARTURE TIME: {}, ARRIVAL TIME: {}, {}, {}",
            departure_time,
            arrival_time,
            is_departure_vertiport_available,
            is_arrival_vertiport_available
        );
        if !is_departure_vertiport_available {
            router_debug!(
                "Departure vertiport not available for departure time {}",
                departure_time
            );
            continue;
        }
        if !is_arrival_vertiport_available {
            router_debug!(
                "Arrival vertiport not available for departure time {}",
                departure_time
            );
            let found_rerouted_vehicle_flight_plan = find_rerouted_vehicle_flight_plan(
                &vehicles_at_arrival_airport,
                &vertiport_arrive,
                &vertipads_arrive,
                &arrival_time,
                &existing_flight_plans,
            );
            if let Some(flight_plan) = found_rerouted_vehicle_flight_plan {
                deadhead_flights.push(flight_plan);
            } else {
                router_debug!("No rerouted vehicle found");
                continue;
            }
        }
        let mut available_vehicle: Option<Vehicle> = None;
        for vehicle in &vehicles {
            router_debug!(
                "Checking vehicle id:{} for departure time: {}",
                &vehicle.id,
                departure_time
            );
            let (vehicle_vertiport_id, minutes_to_arrival) =
                get_vehicle_scheduled_location(vehicle, departure_time, &existing_flight_plans);
            if vehicle_vertiport_id != vertiport_depart.id || minutes_to_arrival > 0 {
                router_debug!(
                    "Vehicle id:{} not available at location for requested time {}. It is/will be at vertiport id: {} in {} minutes",
                    &vehicle.id, departure_time, vehicle_vertiport_id, minutes_to_arrival
                );
                continue;
            }
            let result = is_vehicle_available(
                vehicle,
                departure_time,
                block_aircraft_and_vertiports_minutes as i64,
                &existing_flight_plans,
            );

            let Ok(is_vehicle_available) = result else {
                router_debug!(
                    "Could not determine vehicle availability: (id {}) {}",
                    &vehicle.id, result.unwrap_err()
                );
                continue;
            };

            if !is_vehicle_available {
                router_debug!(
                    "Vehicle id:{} not available for departure time: {} and duration {} minutes",
                    &vehicle.id,
                    departure_time,
                    block_aircraft_and_vertiports_minutes
                );
                continue;
            }
            //when vehicle is available, break the "vehicles" loop early and add flight plan
            available_vehicle = Some(vehicle.clone());
            router_debug!("Found available vehicle with id: {} from vertiport id: {}, for a flight for a departure time {}", &vehicle.id, &vertiport_depart.id,
                        departure_time
                    );
            break;
        }
        // No simple flight plans found, looking for plans with deadhead flights
        if available_vehicle.is_none() {
            router_debug!(
                "No available vehicles for departure time {}, looking for deadhead flights...",
                departure_time
            );

            let (a_vehicle, deadhead_flight_plan) = find_deadhead_flight_plan(
                &nearest_vertiports_from_departure,
                &departure_vertiport_durations,
                &vehicles,
                &vertiport_depart,
                &vertipads_depart,
                departure_time,
                &existing_flight_plans,
                block_aircraft_and_vertiports_minutes as i64,
            );
            if a_vehicle.is_some() {
                available_vehicle = a_vehicle;
                deadhead_flights.push(deadhead_flight_plan.unwrap());
            }
        }
        if available_vehicle.is_none() {
            router_debug!(
                "DH: No available vehicles for departure time {} (including deadhead flights)",
                departure_time
            );
            continue;
        }
        //4. should check other constraints (cargo weight, number of passenger seats)
        //router_info!("[4/5]: Checking other constraints (cargo weight, number of passenger seats)");
        flight_plans.push((
            create_flight_plan_data(
                available_vehicle.unwrap().id.clone(),
                vertiport_depart.id.clone(),
                vertiport_arrive.id.clone(),
                departure_time,
                arrival_time,
            ),
            deadhead_flights,
        ));
    }
    if flight_plans.is_empty() {
        return Err("No flight plans found for given time window".to_string());
    }

    //5. return draft flight plan(s)
    router_info!(
        "[5/5]: Returning {} draft flight plan(s)",
        flight_plans.len()
    );
    router_debug!("Flight plans: {:?}", flight_plans);
    Ok(flight_plans)
}

/// Estimates the time needed to travel between two locations including loading and unloading
/// Estimate should be rather generous to block resources instead of potentially overloading them
pub fn estimate_flight_time_minutes(distance_km: f32, aircraft: Aircraft) -> f32 {
    router_debug!("distance_km: {}", distance_km);
    router_debug!("aircraft: {:?}", aircraft);
    match aircraft {
        Aircraft::Cargo => {
            LOADING_AND_TAKEOFF_TIME_MIN
                + distance_km / AVG_SPEED_KMH * 60.0
                + LANDING_AND_UNLOADING_TIME_MIN
        }
    }
}

/// gets node by id
pub fn get_node_by_id(id: &str) -> Result<&'static Node, String> {
    router_debug!("id: {}", id);
    let nodes = NODES.get().expect("Nodes not initialized");
    let node = nodes
        .iter()
        .find(|node| node.uid == id)
        .ok_or_else(|| "Node not found by id: ".to_owned() + id)?;
    Ok(node)
}

/// Initialize the router with vertiports from the storage service
pub fn init_router_from_vertiports(vertiports: &[Vertiport]) -> Result<(), String> {
    router_info!("Initializing router from vertiports");
    let nodes = vertiports
        .iter()
        .map(|vertiport| Node {
            uid: vertiport.id.clone(),
            location: Location {
                latitude: OrderedFloat(
                    vertiport
                        .data
                        .as_ref()
                        .ok_or_else(|| format!("Something went wrong when parsing latitude data of vertiport id: {}", vertiport.id))
                        .unwrap()
                        .latitude as f32,
                ),
                longitude: OrderedFloat(
                    vertiport
                        .data
                        .as_ref()
                        .ok_or_else(|| format!("Something went wrong when parsing longitude data of vertiport id: {}", vertiport.id))
                        .unwrap()
                        .longitude as f32,
                ),
                altitude_meters: OrderedFloat(0.0),
            },
            forward_to: None,
            status: status::Status::Ok,
            schedule: vertiport
                .data
                .as_ref()
                .ok_or_else(|| format!("Something went wrong when parsing schedule data of vertiport id: {}", vertiport.id))
                .unwrap().schedule.clone(),
        })
        .collect();
    NODES.set(nodes).map_err(|_| "Failed to set NODES")?;
    init_router()
}

#[allow(dead_code)]
/// Takes customer location (src) and required destination (dst) and returns a tuple with nearest vertiports to src and dst
pub fn get_nearest_vertiports<'a>(
    src_location: &'a Location,
    dst_location: &'a Location,
    vertiports: &'static Vec<Node>,
) -> (&'static Node, &'static Node) {
    router_info!("Getting nearest vertiports");
    let mut src_vertiport = &vertiports[0];
    let mut dst_vertiport = &vertiports[0];
    router_debug!("src_location: {:?}", src_location);
    router_debug!("dst_location: {:?}", dst_location);
    let mut src_distance = haversine::distance(src_location, &src_vertiport.location);
    let mut dst_distance = haversine::distance(dst_location, &dst_vertiport.location);
    router_debug!("src_distance: {}", src_distance);
    router_debug!("dst_distance: {}", dst_distance);
    for vertiport in vertiports {
        router_debug!("checking vertiport: {:?}", vertiport);
        let new_src_distance = haversine::distance(src_location, &vertiport.location);
        let new_dst_distance = haversine::distance(dst_location, &vertiport.location);
        router_debug!("new_src_distance: {}", new_src_distance);
        router_debug!("new_dst_distance: {}", new_dst_distance);
        if new_src_distance < src_distance {
            src_distance = new_src_distance;
            src_vertiport = vertiport;
        }
        if new_dst_distance < dst_distance {
            dst_distance = new_dst_distance;
            dst_vertiport = vertiport;
        }
    }
    router_debug!("src_vertiport: {:?}", src_vertiport);
    router_debug!("dst_vertiport: {:?}", dst_vertiport);
    (src_vertiport, dst_vertiport)
}

#[allow(dead_code)]
/// Returns a list of nodes near the given location
pub fn get_nearby_nodes(
    nodes_store: &OnceCell<Vec<Node>>,
    query: NearbyLocationQuery,
) -> &'static Vec<Node> {
    router_debug!("query: {:?}", query);
    let v: Vec<Node> = generate_nodes_near(&query.location, query.radius, query.capacity);

    match nodes_store.set(v) {
        Ok(()) => {}
        Err(_) => {
            router_warn!("(get_nearby_nodes) failed to set node store.");
        }
    }

    match NODES.get() {
        Some(t) => t,
        None => {
            router_error!("(get_nearby nodes) failed to get nodes.");

            // TODO(R3): This will all be refactored, will panic for now.
            panic!();
        }
    }
}

/// Checks if router is initialized
pub fn is_router_initialized() -> bool {
    ARROW_CARGO_ROUTER.get().is_some()
}

/// Get route
pub fn get_route(req: RouteQuery) -> Result<(Vec<Location>, f32), String> {
    router_debug!("Getting route");
    let RouteQuery {
        from,
        to,
        aircraft: _,
    } = req;

    if ARROW_CARGO_ROUTER.get().is_none() {
        return Err("Arrow XL router not initialized. Try to initialize it first.".to_string());
    }
    let result = ARROW_CARGO_ROUTER
        .get()
        .as_ref()
        .ok_or("Can't access router")
        .unwrap()
        .find_shortest_path(from, to, Algorithm::Dijkstra, None);

    let Ok((cost, path)) = result else {
        return Err(format!("{:?}", result.unwrap_err()));
    };

    router_debug!("cost: {}", cost);
    router_debug!("path: {:?}", path);
    let locations = path
        .iter()
        .map(|node_idx| {
            ARROW_CARGO_ROUTER
                .get()
                .as_ref()
                .ok_or("Can't access router")
                .unwrap()
                .get_node_by_id(*node_idx)
                .ok_or(format!("Node not found by index {:?}", *node_idx))
                .unwrap()
                .location
        })
        .collect::<Vec<Location>>();
    router_debug!("locations: {:?}", locations);
    router_info!("Finished getting route with cost: {}", cost);
    Ok((locations, cost))
}

/// Initializes the router for the given aircraft
pub fn init_router() -> Result<(), String> {
    if NODES.get().is_none() {
        return Err("Nodes not initialized. Try to get some nodes first.".to_string());
    }
    if ARROW_CARGO_ROUTER.get().is_some() {
        return Err(
            "Router already initialized. Try to use the router instead of initializing it."
                .to_string(),
        );
    }
    ARROW_CARGO_ROUTER
        .set(Router::new(
            NODES.get().as_ref().unwrap(),
            ARROW_CARGO_CONSTRAINT,
            |from, to| haversine::distance(&from.as_node().location, &to.as_node().location),
            |from, to| haversine::distance(&from.as_node().location, &to.as_node().location),
        ))
        .map_err(|_| "Failed to initialize router".to_string())
}

#[cfg(test)]
mod router_tests {
    use super::{
        get_nearby_nodes, get_nearest_vertiports, get_route, init_router, Aircraft,
        NearbyLocationQuery, RouteQuery, SAN_FRANCISCO,
    };
    use crate::router::router_types::{location::Location, node::Node};
    use once_cell::sync::OnceCell;
    use ordered_float::OrderedFloat;

    #[test]
    fn test_router() {
        pub static NODES: OnceCell<Vec<Node>> = OnceCell::new();

        let nodes = get_nearby_nodes(
            &NODES,
            NearbyLocationQuery {
                location: SAN_FRANCISCO,
                radius: 25.0,
                capacity: 20,
            },
        );

        //router_println!("nodes: {:?}", nodes);
        let init_res = init_router();
        println!("init_res: {:?}", init_res);
        let src_location = Location {
            latitude: OrderedFloat(37.52123),
            longitude: OrderedFloat(-122.50892),
            altitude_meters: OrderedFloat(20.0),
        };
        let dst_location = Location {
            latitude: OrderedFloat(37.81032),
            longitude: OrderedFloat(-122.28432),
            altitude_meters: OrderedFloat(20.0),
        };
        let (src, dst) = get_nearest_vertiports(&src_location, &dst_location, nodes);
        println!("src: {:?}, dst: {:?}", src.location, dst.location);
        let (route, cost) = get_route(RouteQuery {
            from: src,
            to: dst,
            aircraft: Aircraft::Cargo,
        })
        .unwrap();
        println!("route: {:?}", route);
        assert!(route.len() > 0, "Route should not be empty");
        assert!(cost > 0.0, "Cost should be greater than 0");
    }
}
