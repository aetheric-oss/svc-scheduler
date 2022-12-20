//! Implementation of the queries/actions that the scheduler service can perform.
use crate::scheduler_grpc::{
    CancelFlightResponse, ConfirmFlightResponse, FlightPriority, FlightStatus, Id, QueryFlightPlan,
    QueryFlightPlanBundle, QueryFlightRequest, QueryFlightResponse,
};
use once_cell::sync::OnceCell;
use prost_types::{FieldMask, Timestamp};
use router::router_state::get_possible_flights;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;
use svc_storage_client_grpc::client::{
    FlightPlan, FlightPlanData, Id as StorageId, SearchFilter, UpdateFlightPlan,
};

use crate::grpc_client_wrapper::StorageClientWrapperTrait;
use tokio;
use tonic::{Request, Response, Status};
use uuid::Uuid;

const CANCEL_FLIGHT_SECONDS: u64 = 30;

/// gets or creates a new hashmap of unconfirmed flight plans
fn unconfirmed_flight_plans() -> &'static Mutex<HashMap<String, FlightPlanData>> {
    static INSTANCE: OnceCell<Mutex<HashMap<String, FlightPlanData>>> = OnceCell::new();
    INSTANCE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// spawns a thread that will cancel the flight plan after a certain amount of time (CANCEL_FLIGHT_SECONDS)
fn cancel_flight_after_timeout(id: String) {
    tokio::spawn(async move {
        tokio::time::sleep(core::time::Duration::from_secs(CANCEL_FLIGHT_SECONDS)).await;
        let mut flight_plans = unconfirmed_flight_plans()
            .lock()
            .expect("Mutex Lock Error removing flight plan after timeout");
        if flight_plans.get(&id).is_some() {
            debug!("Flight plan {} was not confirmed in time, cancelling", id);
            flight_plans.remove(&id);
        };
    });
}

///Finds the first possible flight for customer location, flight type and requested time.
pub async fn query_flight(
    request: Request<QueryFlightRequest>,
    storage_client_wrapper: &(dyn StorageClientWrapperTrait + Send + Sync),
) -> Result<Response<QueryFlightResponse>, Status> {
    let flight_request = request.into_inner();
    info!(
        "query_flight with vertiport depart, arrive ids: {}, {}",
        &flight_request.vertiport_depart_id, &flight_request.vertiport_arrive_id
    );
    let depart_vertiport = storage_client_wrapper
        .vertiport_by_id(Request::new(StorageId {
            id: flight_request.vertiport_depart_id,
        }))
        .await?
        .into_inner();
    let arrive_vertiport = storage_client_wrapper
        .vertiport_by_id(Request::new(StorageId {
            id: flight_request.vertiport_arrive_id,
        }))
        .await?
        .into_inner();
    debug!(
        "depart_vertiport: {:?}, arrive_vertiport: {:?}",
        &depart_vertiport, &arrive_vertiport
    );
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
    debug!(
        "departure_vertiport_flights: {}, arrival_vertiport_flights: {}",
        &departure_vertiport_flights.len(),
        &arrival_vertiport_flights.len()
    );
    if !departure_vertiport_flights.is_empty() {
        return Err(Status::not_found("Departure vertiport not available"));
    }
    if !arrival_vertiport_flights.is_empty() {
        return Err(Status::not_found("Arrival vertiport not available"));
    }
    //5. check schedule of aircrafts
    let aircrafts = storage_client_wrapper
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
        flight_request.earliest_departure_time,
        flight_request.latest_arrival_time,
        aircrafts,
    );
    if flight_plans.is_err() || flight_plans.as_ref().unwrap().is_empty() {
        return Err(Status::not_found(
            "No flight plans available; ".to_owned() + &flight_plans.err().unwrap(),
        ));
    }
    let flight_plans = flight_plans.unwrap();
    info!("Found  {} flight plans from router", &flight_plans.len());
    let fp = flight_plans.first().unwrap();
    //7. create draft flight plan (in memory)
    let fp_id = Uuid::new_v4().to_string();
    info!(
        "Adding draft flight plan with temporary id: {} with timeout {} seconds",
        &fp_id, CANCEL_FLIGHT_SECONDS
    );
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
        estimated_distance: fp.flight_distance as u32,
    };
    debug!("query_flight response: {:?}", &item);
    let response = QueryFlightResponse {
        flights: [QueryFlightPlanBundle {
            flight_plan: Some(item),
            deadhead_flight_plans: vec![],
        }]
        .to_vec(),
    };
    info!(
        "query_flight returning: {} flight plans",
        &response.flights.len()
    );
    Ok(Response::new(response))
}

/// Gets flight plan from hash map of unconfirmed flight plans
fn get_fp_by_id(id: String) -> Option<FlightPlanData> {
    unconfirmed_flight_plans()
        .lock()
        .expect("Mutex Lock Error getting flight plan from temp storage")
        .get(&id)
        .cloned()
}

/// Removes flight plan from hash map of unconfirmed flight plans
fn remove_fp_by_id(id: String) -> bool {
    let mut flight_plans = unconfirmed_flight_plans()
        .lock()
        .expect("Mutex Lock Error removing flight plan from temp storage");
    let found = flight_plans.get(&id).is_some();
    if found {
        flight_plans.remove(&id);
        info!("cancel_flight with id {} removed from local cache", &id);
    }
    found
}

///Confirms the flight plan
pub async fn confirm_flight(
    request: Request<Id>,
    storage_client_wrapper: &(dyn StorageClientWrapperTrait + Send + Sync),
) -> Result<Response<ConfirmFlightResponse>, Status> {
    let fp_id = request.into_inner().id;
    info!("confirm_flight with id {}", &fp_id);
    let draft_fp = get_fp_by_id(fp_id.clone());
    return if draft_fp.is_none() {
        Err(Status::not_found("Flight plan not found"))
    } else {
        let fp = storage_client_wrapper
            .insert_flight_plan(Request::new(draft_fp.unwrap()))
            .await?
            .into_inner();
        let sys_time = SystemTime::now();
        info!("confirm_flight: Flight plan with draft id: {} inserted in to storage with permanent id:{}", &fp_id, &fp.id);
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
    storage_client_wrapper: &(dyn StorageClientWrapperTrait + Send + Sync),
) -> Result<Response<CancelFlightResponse>, Status> {
    let fp_id = request.into_inner().id;
    info!("cancel_flight with id {}", &fp_id);
    let mut found = remove_fp_by_id(fp_id.clone());
    if !found {
        let fp = storage_client_wrapper
            .flight_plan_by_id(Request::new(StorageId { id: fp_id.clone() }))
            .await;
        found = fp.is_ok();
        if found {
            storage_client_wrapper
                .update_flight_plan(Request::new(UpdateFlightPlan {
                    id: fp_id.clone(),
                    data: Option::from(FlightPlanData {
                        pilot_id: "".to_string(),
                        vehicle_id: "".to_string(),
                        cargo_weight_g: vec![],
                        flight_distance: 0,
                        weather_conditions: "".to_string(),
                        departure_vertiport_id: Some("".to_string()),
                        departure_vertipad_id: "".to_string(),
                        destination_vertiport_id: Some("".to_string()),
                        destination_vertipad_id: "".to_string(),
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
            info!("cancel_flight with id {} cancelled in storage", &fp_id);
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
        let err_msg = format!(
            "cancel_flight with id {} not found neither in local cache nor storage",
            &fp_id
        );
        Err(Status::not_found(err_msg))
    }
}

#[cfg(test)]
mod tests {
    mod test_utils {
        include!("test_utils.rs");
    }

    use super::*;
    use chrono::{TimeZone, Utc};
    use router::router_state::{init_router_from_vertiports, NODES};
    use test_utils::*;

    #[tokio::test]
    async fn test_storage_client_stub() {
        let client_wrapper = create_storage_client_stub();
        let id = "vertiport1".to_string();
        let response = client_wrapper
            .vertiport_by_id(Request::new(StorageId { id: id.clone() }))
            .await
            .unwrap()
            .into_inner();
        // Validate server response with assertions
        assert_eq!(response.id, id);
    }

    #[tokio::test]
    async fn test_query_flight() {
        let client_wrapper = create_storage_client_stub();
        let timestamp = Utc
            .with_ymd_and_hms(2022, 10, 25, 15, 0, 0)
            .unwrap()
            .timestamp();

        let vertiports = client_wrapper
            .vertiports(Request::new(SearchFilter {
                search_field: "".to_string(),
                search_value: "".to_string(),
                page_number: 0,
                results_per_page: 0,
            }))
            .await
            .unwrap()
            .into_inner()
            .vertiports;
        let init_res = init_router_from_vertiports(&vertiports);
        assert_eq!(NODES.get().unwrap().len(), 2);
        let res = query_flight(
            Request::new(QueryFlightRequest {
                is_cargo: false,
                persons: None,
                weight_grams: None,
                departure_time: Some(Timestamp {
                    seconds: timestamp,
                    nanos: 0,
                }),
                arrival_time: None,
                vertiport_depart_id: "vertiport1".to_string(),
                vertiport_arrive_id: "vertiport2".to_string(),
            }),
            &client_wrapper,
        )
        .await
        .unwrap();
        assert_eq!(res.into_inner().flights.len(), 1);
    }
}
