use crate::router_utils::{
    estimate_flight_time_minutes, get_nearby_nodes, get_route, init_router, Aircraft,
    NearbyLocationQuery, RouteQuery, LANDING_AND_UNLOADING_TIME_MIN, LOADING_AND_TAKEOFF_TIME_MIN,
    NODES, SAN_FRANCISCO,
};
use crate::scheduler_grpc::{
    CancelFlightResponse, ConfirmFlightResponse, FlightPriority, FlightStatus, Id, QueryFlightPlan,
    QueryFlightRequest, QueryFlightResponse,
};
use std::collections::HashMap;

use crate::calendar_utils::{Calendar, Tz};
use chrono::{Duration, NaiveDateTime, TimeZone};
use once_cell::sync::OnceCell;
use prost_types::Timestamp;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::SystemTime;
use svc_storage_client_grpc::client::{
    storage_rpc_client::StorageRpcClient, AircraftFilter, FlightPlan, VertiportFilter,
};
use tokio;
use tonic::{Request, Response, Status};
use uuid::Uuid;

const CANCEL_FLIGHT_SECONDS: u64 = 30;

fn unconfirmed_flight_plans() -> &'static Mutex<HashMap<String, FlightPlan>> {
    static INSTANCE: OnceCell<Mutex<HashMap<String, FlightPlan>>> = OnceCell::new();
    INSTANCE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cancel_flight_after_timeout(id: String) {
    tokio::spawn(async move {
        tokio::time::sleep(core::time::Duration::from_secs(CANCEL_FLIGHT_SECONDS)).await;
        let mut flight_plans = unconfirmed_flight_plans().lock().unwrap();
        if flight_plans.get(&id).is_some() {
            flight_plans.remove(&id);
        };
    });
}

///Finds the first possible flight for customer location, flight type and requested time.
pub async fn query_flight(
    request: Request<QueryFlightRequest>,
    mut storage_client: StorageRpcClient<tonic::transport::Channel>,
) -> Result<Response<QueryFlightResponse>, Status> {
    let flight_request = request.into_inner();
    // 1. Fetch vertiports from customer request
    let _r_vertiports = storage_client
        .vertiports(Request::new(VertiportFilter {}))
        .await?
        .into_inner()
        .vertiports;
    //TODO use vertiports from DB instead of NODES
    //2. Find route and cost between requested vertiports
    if NODES.get().is_none() {
        get_nearby_nodes(NearbyLocationQuery {
            location: SAN_FRANCISCO,
            radius: 25.0,
            capacity: 20,
        });
        init_router();
    }
    let (route, cost) = get_route(RouteQuery {
        from: &NODES.get().unwrap()[0],
        to: &NODES.get().unwrap()[1],
        aircraft: Aircraft::Cargo,
    })
    .unwrap();
    println!("route distance: {:?}", cost);
    if route.is_empty() {
        return Err(Status::not_found("Route between vertiports not found"));
    }
    //3. calculate blocking times for each vertiport and aircraft
    let block_departure_vertiport_minutes = LOADING_AND_TAKEOFF_TIME_MIN;
    let block_arrival_vertiport_minutes = LANDING_AND_UNLOADING_TIME_MIN;
    let block_aircraft_minutes = estimate_flight_time_minutes(cost, Aircraft::Cargo);

    //4. check vertiport schedules and flight plans
    const SAMPLE_CAL: &str =
        "DTSTART:20221020T180000Z;DURATION:PT1H\nRRULE:FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR";
    let departure_vertiport_schedule = Calendar::from_str(SAMPLE_CAL).unwrap(); //TODO get from DB
    let arrival_vertiport_schedule = Calendar::from_str(SAMPLE_CAL).unwrap(); //TODO get from DB

    if flight_request.departure_time.is_none() && flight_request.arrival_time.is_none() {
        return Err(Status::invalid_argument(
            "Either departure_time or arrival_time must be set",
        ));
    }

    let (departure_time, arrival_time) = if flight_request.departure_time.is_some() {
        let departure_time = Tz::UTC.from_utc_datetime(&NaiveDateTime::from_timestamp(
            flight_request.departure_time.as_ref().unwrap().seconds,
            flight_request.departure_time.as_ref().unwrap().nanos as u32,
        ));
        (
            departure_time,
            departure_time + Duration::minutes(block_aircraft_minutes as i64),
        )
    } else {
        let arrival_time = Tz::UTC.from_utc_datetime(&NaiveDateTime::from_timestamp(
            flight_request.arrival_time.as_ref().unwrap().seconds,
            flight_request.arrival_time.as_ref().unwrap().nanos as u32,
        ));
        (
            arrival_time - Duration::minutes(block_aircraft_minutes as i64),
            arrival_time,
        )
    };
    let is_departure_vertiport_available = departure_vertiport_schedule.is_available_between(
        departure_time,
        departure_time + Duration::minutes(block_departure_vertiport_minutes as i64),
    );
    let departure_vertiport_flights: Vec<FlightPlan> = vec![];
    /*todo storage_client
    .flight_plans(Request::new(FlightPlanFilter {})) //todo filter flight_plans(estimated_departure_between: ($from, $to), vertiport_id: $ID)
    .await?
    .into_inner()
    .flight_plans;*/

    if !is_departure_vertiport_available || !departure_vertiport_flights.is_empty() {
        return Err(Status::not_found("Departure vertiport not available"));
    }

    let is_arrival_vertiport_available = arrival_vertiport_schedule.is_available_between(
        arrival_time - Duration::minutes(block_arrival_vertiport_minutes as i64),
        arrival_time,
    );
    if !is_arrival_vertiport_available {
        return Err(Status::not_found("Arrival vertiport not available"));
    }
    let arrival_vertiport_flights: Vec<FlightPlan> = vec![];
    /* todo storage_client
    .flight_plans(Request::new(FlightPlanFilter {}))
    //todo filter flight_plans(estimated_arrival_between: ($from, $to), vertiport_id: $ID)
    .await?
    .into_inner()
    .flight_plans;*/
    if !is_arrival_vertiport_available || !arrival_vertiport_flights.is_empty() {
        return Err(Status::not_found("Arrival vertiport not available"));
    }
    //5. check schedule of aircrafts
    let aircrafts = storage_client
        .aircrafts(Request::new(AircraftFilter {})) //todo filter associated aircrafts to dep vertiport?
        .await?
        .into_inner()
        .aircrafts;
    for _aircraft in aircrafts {
        let aircraft_schedule = Calendar::from_str(SAMPLE_CAL).unwrap(); //TODO get from aircraft.schedule
        let is_aircraft_available =
            aircraft_schedule.is_available_between(departure_time, arrival_time);
        let aircraft_flights: Vec<FlightPlan> = vec![];
        /*todo storage_client
        .flight_plans(Request::new(FlightPlanFilter {}))
        //todo filter flight_plans(estimated_departure_between: ($from, $to), estimated_arrival_between: ($from2, $to2) aircraft_id: $ID)
        .await?
        .into_inner()
        .flight_plans;*/
        if !is_aircraft_available || !aircraft_flights.is_empty() {
            return Err(Status::not_found("Aircraft not available"));
        }
    }
    //6. TODO: check other constraints (cargo weight, number of passenger seats)
    //7. create draft flight plan (in memory)
    let fp_id = Uuid::new_v4().to_string();
    unconfirmed_flight_plans().lock().unwrap().insert(
        fp_id.clone(),
        FlightPlan {
            id: 1234, //todo string id
            flight_status: FlightStatus::Ready as i32,
        },
    );
    //8. automatically cancel draft flight plan if not confirmed by user
    cancel_flight_after_timeout(fp_id.clone());
    //9. return response - TODO copy from storage flight plan
    let item = QueryFlightPlan {
        id: fp_id,
        pilot_id: 1,
        aircraft_id: 1,
        cargo: [123].to_vec(),
        weather_conditions: "Sunny, no wind :)".to_string(),
        vertiport_id_departure: 1,
        pad_id_departure: 1,
        vertiport_id_destination: 1,
        pad_id_destination: 1,
        estimated_departure: Some(Timestamp {
            seconds: departure_time.timestamp(),
            nanos: departure_time.timestamp_subsec_nanos() as i32,
        }),
        estimated_arrival: Some(Timestamp {
            seconds: arrival_time.timestamp(),
            nanos: arrival_time.timestamp_subsec_nanos() as i32,
        }),
        actual_departure: None,
        actual_arrival: None,
        flight_release_approval: None,
        flight_plan_submitted: None,
        flight_status: FlightStatus::Ready as i32,
        flight_priority: FlightPriority::Low as i32,
    };
    let response = QueryFlightResponse {
        flights: [item].to_vec(),
    };
    Ok(Response::new(response))
}

fn get_fp_by_id(id: String) -> Option<FlightPlan> {
    unconfirmed_flight_plans().lock().unwrap().get(&id).copied()
}

///Confirms the flight plan
pub async fn confirm_flight(
    request: Request<Id>,
    mut storage_client: StorageRpcClient<tonic::transport::Channel>,
) -> Result<Response<ConfirmFlightResponse>, Status> {
    let fp_id = request.into_inner().id;
    let draft_fp = get_fp_by_id(fp_id.clone());
    return if draft_fp.is_none() {
        Err(Status::not_found("Flight plan not found"))
    } else {
        let fp = storage_client
            .insert_flight_plan(Request::new(draft_fp.unwrap()))
            .await?
            .into_inner();
        let sys_time = SystemTime::now();
        let response = ConfirmFlightResponse {
            id: fp.id.to_string(), //todo this should be string
            confirmed: true,
            confirmation_time: Some(Timestamp::from(sys_time)),
        };
        unconfirmed_flight_plans().lock().unwrap().remove(&fp_id);
        Ok(Response::new(response))
    };
}

/// Cancels a draft flight plan
pub async fn cancel_flight(request: Request<Id>) -> Result<Response<CancelFlightResponse>, Status> {
    let fp_id = request.into_inner().id;
    let mut flight_plans = unconfirmed_flight_plans().lock().unwrap();
    if flight_plans.get(&fp_id).is_some() {
        flight_plans.remove(&fp_id);
    };
    let sys_time = SystemTime::now();
    let response = CancelFlightResponse {
        id: fp_id,
        cancelled: true,
        cancellation_time: Some(Timestamp::from(sys_time)),
        reason: "user cancelled".into(),
    };
    Ok(Response::new(response))
}
