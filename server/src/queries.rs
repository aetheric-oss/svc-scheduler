use crate::scheduler_grpc::{
    CancelFlightResponse, ConfirmFlightResponse, FlightPriority, FlightStatus, Id, QueryFlightPlan,
    QueryFlightRequest, QueryFlightResponse,
};
use once_cell::sync::OnceCell;
use prost_types::{FieldMask, Timestamp};
use router::router_state::get_possible_flights;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;
use svc_storage_client_grpc::client::{
    flight_plan_rpc_client::FlightPlanRpcClient, vehicle_rpc_client::VehicleRpcClient,
    vertiport_rpc_client::VertiportRpcClient, FlightPlan, FlightPlanData, Id as StorageId,
    SearchFilter, UpdateFlightPlan,
};

use tokio;
use tonic::{Request, Response, Status};
use uuid::Uuid;

const CANCEL_FLIGHT_SECONDS: u64 = 30;

fn unconfirmed_flight_plans() -> &'static Mutex<HashMap<String, FlightPlanData>> {
    static INSTANCE: OnceCell<Mutex<HashMap<String, FlightPlanData>>> = OnceCell::new();
    INSTANCE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cancel_flight_after_timeout(id: String) {
    tokio::spawn(async move {
        tokio::time::sleep(core::time::Duration::from_secs(CANCEL_FLIGHT_SECONDS)).await;
        let mut flight_plans = unconfirmed_flight_plans()
            .lock()
            .expect("Mutex Lock Error removing flight plan after timeout");
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
    let depart_vertiport = vertiport_client
        .vertiport_by_id(Request::new(StorageId {
            id: flight_request.vertiport_depart_id,
        }))
        .await?
        .into_inner();
    let arrive_vertiport = vertiport_client
        .vertiport_by_id(Request::new(StorageId {
            id: flight_request.vertiport_arrive_id,
        }))
        .await?
        .into_inner();
    let departure_vertiport_flights: Vec<FlightPlan> = vec![];
    /*todo storage_client
    .flight_plans(Request::new(FlightPlanFilter {})) //todo filter flight_plans(estimated_departure_between: ($from, $to), vertiport_id: $ID)
    .await?
    .into_inner()
    .flight_plans;*/
    let arrival_vertiport_flights: Vec<FlightPlan> = vec![];
    /* todo storage_client
    .flight_plans(Request::new(FlightPlanFilter {}))
    //todo filter flight_plans(estimated_arrival_between: ($from, $to), vertiport_id: $ID)
    .await?
    .into_inner()
    .flight_plans;*/

    if !departure_vertiport_flights.is_empty() {
        return Err(Status::not_found("Departure vertiport not available"));
    }
    if !arrival_vertiport_flights.is_empty() {
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
    for _aircraft in &aircrafts {
        let aircraft_flights: Vec<FlightPlan> = vec![];
        /*todo storage_client
        .flight_plans(Request::new(FlightPlanFilter {}))
        //todo filter flight_plans(estimated_departure_between: ($from, $to), estimated_arrival_between: ($from2, $to2) aircraft_id: $ID)
        .await?
        .into_inner()
        .flight_plans;*/
        if !aircraft_flights.is_empty() {
            return Err(Status::not_found("Aircraft not available"));
        }
    }
    let flight_plans = get_possible_flights(
        depart_vertiport,
        arrive_vertiport,
        flight_request.departure_time,
        flight_request.arrival_time,
        aircrafts,
    );
    if flight_plans.is_err() || flight_plans.as_ref().unwrap().is_empty() {
        return Err(Status::not_found("No flight plans available"));
    }
    let flight_plans = flight_plans.unwrap();
    let fp = flight_plans.first().unwrap();
    //7. create draft flight plan (in memory)
    let fp_id = Uuid::new_v4().to_string();
    unconfirmed_flight_plans()
        .lock()
        .expect("Mutex Lock Error inserting flight plan into temp storage")
        .insert(fp_id.clone(), fp.clone());

    //8. automatically cancel draft flight plan if not confirmed by user
    cancel_flight_after_timeout(fp_id.clone());
    //9. return response - TODO copy from storage flight plan
    let item = QueryFlightPlan {
        id: fp_id,
        pilot_id: "1".to_string(),
        vehicle_id: "1".to_string(),
        cargo: [123].to_vec(),
        weather_conditions: "Sunny, no wind :)".to_string(),
        vertiport_depart_id: "1".to_string(),
        pad_depart_id: "1".to_string(),
        vertiport_arrive_id: "1".to_string(),
        pad_arrive_id: "1".to_string(),
        estimated_departure: fp.clone().scheduled_departure,
        estimated_arrival: fp.clone().scheduled_arrival,
        actual_departure: None,
        actual_arrival: None,
        flight_release_approval: None,
        flight_plan_submitted: None,
        flight_status: FlightStatus::Ready as i32,
        flight_priority: FlightPriority::Low as i32,
        estimated_distance: fp.flight_distance,
    };
    let response = QueryFlightResponse {
        flights: [item].to_vec(),
    };
    Ok(Response::new(response))
}

fn get_fp_by_id(id: String) -> Option<FlightPlanData> {
    unconfirmed_flight_plans()
        .lock()
        .expect("Mutex Lock Error getting flight plan from temp storage")
        .get(&id)
        .cloned()
}

fn remove_fp_by_id(id: String) -> bool {
    let mut flight_plans = unconfirmed_flight_plans()
        .lock()
        .expect("Mutex Lock Error removing flight plan from temp storage");
    let found = flight_plans.get(&id).is_some();
    if found {
        flight_plans.remove(&id);
    }
    found
}

///Confirms the flight plan
pub async fn confirm_flight(
    request: Request<Id>,
    mut storage_client: FlightPlanRpcClient<tonic::transport::Channel>,
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
            id: fp.id,
            confirmed: true,
            confirmation_time: Some(Timestamp::from(sys_time)),
        };
        match unconfirmed_flight_plans().lock() {
            Ok(mut flight_plans) => {
                flight_plans.remove(&fp_id);
            }
            Err(e) => {
                return Err(Status::internal(format!(
                    "Failed to remove flight plan from unconfirmed list: {}",
                    e
                )));
            }
        }
        Ok(Response::new(response))
    };
}

/// Cancels a draft or confirmed flight plan
pub async fn cancel_flight(
    request: Request<Id>,
    mut storage_client: FlightPlanRpcClient<tonic::transport::Channel>,
) -> Result<Response<CancelFlightResponse>, Status> {
    let fp_id = request.into_inner().id;
    let mut found = remove_fp_by_id(fp_id.clone());
    if !found {
        let fp = storage_client
            .flight_plan_by_id(Request::new(StorageId { id: fp_id.clone() }))
            .await;
        found = fp.is_ok();
        if found {
            storage_client
                .update_flight_plan(Request::new(UpdateFlightPlan {
                    id: "".to_string(),
                    data: Option::from(FlightPlanData {
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
                        flight_status: FlightStatus::Cancelled as i32,
                        flight_priority: 0,
                    }),
                    mask: Some(FieldMask {
                        paths: vec!["flight_status".to_string()],
                    }),
                }))
                .await?;
        }
    }
    if found {
        let sys_time = SystemTime::now();
        let response = CancelFlightResponse {
            id: fp_id,
            cancelled: true,
            cancellation_time: Some(Timestamp::from(sys_time)),
            reason: "user cancelled".into(),
        };
        Ok(Response::new(response))
    } else {
        Err(Status::not_found("Flight plan not found"))
    }
}
