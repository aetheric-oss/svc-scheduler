use crate::router_utils::{
    estimate_flight_time_minutes, get_node_by_id, get_route, init_router_from_vertiports,
    is_router_initialized, Aircraft, RouteQuery, LANDING_AND_UNLOADING_TIME_MIN,
    LOADING_AND_TAKEOFF_TIME_MIN,
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
    flight_plan_rpc_client::FlightPlanRpcClient, vehicle_rpc_client::VehicleRpcClient,
    vertiport_rpc_client::VertiportRpcClient, FlightPlan, FlightPlanData, SearchFilter,
};

use tokio;
use tonic::{Request, Response, Status};
use uuid::Uuid;

const CANCEL_FLIGHT_SECONDS: u64 = 30;

fn unconfirmed_flight_plans() -> &'static Mutex<HashMap<String, FlightPlanData>> {
    static INSTANCE: OnceCell<Mutex<HashMap<String, FlightPlanData>>> = OnceCell::new();
    let ret = INSTANCE.get_or_init(|| Mutex::new(HashMap::new()));

    ret.lock().unwrap().insert(
        "0fc37762-c423-417c-94bc-5d6d452322d7".to_string(),
        FlightPlanData {
            pilot_id: "".to_string(),
            vehicle_id: "".to_string(),
            cargo_weight: vec![],
            flight_distance: 0,
            weather_conditions: "".to_string(),
            departure_vertiport_id: "".to_string(),
            departure_pad_id: "".to_string(),
            destination_vertiport_id: "".to_string(),
            destination_pad_id: "".to_string(),
            scheduled_departure: None,
            scheduled_arrival: None,
            actual_departure: None,
            actual_arrival: None,
            flight_release_approval: None,
            flight_plan_submitted: None,
            approved_by: None,
            flight_status: 0,
            flight_priority: 0,
        },
    );

    ret
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
    mut _fp_client: FlightPlanRpcClient<tonic::transport::Channel>,
    mut vehicle_client: VehicleRpcClient<tonic::transport::Channel>,
    mut vertiport_client: VertiportRpcClient<tonic::transport::Channel>,
) -> Result<Response<QueryFlightResponse>, Status> {
    let flight_request = request.into_inner();
    // 1. Fetch vertiports from customer request
    let vertiports = vertiport_client
        .vertiports(Request::new(SearchFilter {
            search_field: "".to_string(),
            search_value: "".to_string(),
            page_number: 0,
            results_per_page: 0,
        }))
        .await?
        .into_inner()
        .vertiports;
    println!("Vertiports found: {}", vertiports.len());
    //2. Find route and cost between requested vertiports
    if !is_router_initialized() {
        init_router_from_vertiports(&vertiports);
    }
    let (route, cost) = get_route(RouteQuery {
        from: get_node_by_id(&flight_request.vertiport_depart_id).unwrap(),
        to: get_node_by_id(&flight_request.vertiport_arrive_id).unwrap(),
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
    let aircrafts = vehicle_client
        .vehicles(Request::new(SearchFilter {
            search_field: "".to_string(),
            search_value: "".to_string(),
            page_number: 0,
            results_per_page: 50,
        })) //todo filter associated aircrafts to dep vertiport?
        .await?
        .into_inner()
        .vehicles;
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
        FlightPlanData {
            pilot_id: "".to_string(),
            vehicle_id: "".to_string(),
            cargo_weight: vec![],
            flight_distance: 0,
            weather_conditions: "".to_string(),
            departure_vertiport_id: "".to_string(),
            departure_pad_id: "".to_string(),
            destination_vertiport_id: "".to_string(),
            destination_pad_id: "".to_string(),
            scheduled_departure: None,
            scheduled_arrival: None,
            actual_departure: None,
            actual_arrival: None,
            flight_release_approval: None,
            flight_plan_submitted: None,
            approved_by: None,
            flight_status: 0,
            flight_priority: 0,
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
    println!("{:?}", response);
    Ok(Response::new(response))
}

fn get_fp_by_id(id: String) -> Option<FlightPlanData> {
    unconfirmed_flight_plans().lock().unwrap().get(&id).cloned()
}

///Confirms the flight plan
pub async fn confirm_flight(
    request: Request<Id>,
    mut storage_client: FlightPlanRpcClient<tonic::transport::Channel>,
) -> Result<Response<ConfirmFlightResponse>, Status> {
    let fp_id = request.into_inner().id;
    let draft_fp = get_fp_by_id(fp_id.clone());
    return if draft_fp.is_none() {
        println!("Not found");
        Err(Status::not_found("Flight plan not found"))
    } else {
        println!(" found");

        let fp = storage_client
            .insert_flight_plan(Request::new(draft_fp.unwrap()))
            .await?
            .into_inner();
        let sys_time = SystemTime::now();
        let response = ConfirmFlightResponse {
            id: fp.id, //todo this should be string
            confirmed: true,
            confirmation_time: Some(Timestamp::from(sys_time)),
        };
        unconfirmed_flight_plans().lock().unwrap().remove(&fp_id);
        println!("{:?}", response);

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
    println!("{:?}", response);
    Ok(Response::new(response))
}
