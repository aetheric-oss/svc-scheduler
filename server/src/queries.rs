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
use svc_compliance_client_grpc::client::FlightPlanRequest;
use svc_storage_client_grpc::client::{
    AdvancedSearchFilter, FilterOption, Id as StorageId, SearchFilter,
};
use svc_storage_client_grpc::flight_plan::{
    Data as FlightPlanData, Object as FlightPlan, UpdateObject as UpdateFlightPlan,
};

use crate::grpc_client_wrapper::{ComplianceClientWrapperTrait, StorageClientWrapperTrait};
use tokio;
use tonic::{Request, Response, Status};
use uuid::Uuid;

const CANCEL_FLIGHT_SECONDS: u64 = 30;

/*fn empty_filter() -> SearchFilter {
    SearchFilter {
        search_field: "".to_string(),
        search_value: "".to_string(),
        page_number: 0,
        results_per_page: 0,
    }
}*/

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
    // 1. get vertiports
    info!(
        "query_flight with vertiport depart, arrive ids: {}, {}",
        &flight_request.vertiport_depart_id, &flight_request.vertiport_arrive_id
    );
    let depart_vertiport = storage_client_wrapper
        .vertiport_by_id(Request::new(StorageId {
            id: flight_request.vertiport_depart_id.clone(),
        }))
        .await?
        .into_inner();
    let arrive_vertiport = storage_client_wrapper
        .vertiport_by_id(Request::new(StorageId {
            id: flight_request.vertiport_arrive_id.clone(),
        }))
        .await?
        .into_inner();
    debug!(
        "depart_vertiport: {:?}, arrive_vertiport: {:?}",
        &depart_vertiport, &arrive_vertiport
    );
    //2. get all aircrafts
    let aircrafts = storage_client_wrapper
        .vehicles(Request::new(AdvancedSearchFilter {
            filters: vec![],
            page_number: 0,
            results_per_page: 0,
            order_by: vec![],
        }))
        .await?
        .into_inner()
        .list;
    //3. get all flight plans from this time to latest departure time (including partially fitting flight plans)
    //plans are used by lib_router to find aircraft and vertiport availability and aircraft predicted location
    let timestamp_now = prost_types::Timestamp::from(SystemTime::now());
    let existing_flight_plans = storage_client_wrapper
        .flight_plans(Request::new(
            AdvancedSearchFilter::search_between(
                "scheduled_departure".to_owned(),
                timestamp_now.to_string(),
                flight_request
                    .latest_arrival_time
                    .clone()
                    .unwrap()
                    .to_string(),
            )
            .and_is_not_null("deleted_at".to_owned()),
        ))
        .await?
        .into_inner()
        .list;
    //4. get all possible flight plans from router
    let flight_plans = get_possible_flights(
        depart_vertiport,
        arrive_vertiport,
        flight_request.earliest_departure_time,
        flight_request.latest_arrival_time,
        aircrafts,
        existing_flight_plans,
    );
    if flight_plans.is_err() || flight_plans.as_ref().unwrap().is_empty() {
        return Err(Status::not_found(
            "No flight plans available; ".to_owned() + &flight_plans.err().unwrap(),
        ));
    }
    let flight_plans = flight_plans.unwrap();
    info!("Found  {} flight plans from router", &flight_plans.len());

    //5. create draft flight plans (in memory)
    let mut flights: Vec<QueryFlightPlanBundle> = vec![];
    for fp in &flight_plans {
        let fp_id = Uuid::new_v4().to_string();
        info!(
            "Adding draft flight plan with temporary id: {} with timeout {} seconds",
            &fp_id, CANCEL_FLIGHT_SECONDS
        );
        unconfirmed_flight_plans()
            .lock()
            .expect("Mutex Lock Error inserting flight plan into temp storage")
            .insert(fp_id.clone(), fp.clone());

        //6. automatically cancel draft flight plan if not confirmed by user
        cancel_flight_after_timeout(fp_id.clone());
        let item = QueryFlightPlan {
            id: fp_id,
            pilot_id: fp.pilot_id.clone(),
            vehicle_id: fp.vehicle_id.clone(),
            cargo: [123].to_vec(),
            weather_conditions: fp.weather_conditions.clone().unwrap_or_default(),
            vertiport_depart_id: fp.departure_vertiport_id.clone().unwrap(),
            pad_depart_id: fp.departure_vertipad_id.clone(),
            vertiport_arrive_id: fp.destination_vertiport_id.clone().unwrap(),
            pad_arrive_id: fp.destination_vertipad_id.clone(),
            estimated_departure: fp.scheduled_departure.clone(),
            estimated_arrival: fp.scheduled_arrival.clone(),
            actual_departure: None,
            actual_arrival: None,
            flight_release_approval: None,
            flight_plan_submitted: None,
            flight_status: FlightStatus::Ready as i32,
            flight_priority: FlightPriority::Low as i32,
            estimated_distance: 0,
        };
        debug!("flight plan: {:?}", &item);
        flights.push(QueryFlightPlanBundle {
            flight_plan: Some(item),
            deadhead_flight_plans: vec![],
        });
    }

    //7. return response
    let response = QueryFlightResponse { flights };
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
    compliance_client_wrapper: &(dyn ComplianceClientWrapperTrait + Send + Sync),
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
            .into_inner()
            .object
            .unwrap();
        let sys_time = SystemTime::now();
        info!("confirm_flight: Flight plan with draft id: {} inserted in to storage with permanent id:{}", &fp_id, &fp.id);
        let compliance_res = compliance_client_wrapper
            .submit_flight_plan(Request::new(FlightPlanRequest {
                flight_plan_id: fp.id.clone(),
                data: "".to_string(),
            }))
            .await?
            .into_inner();
        info!(
            "confirm_flight: Compliance response for flight plan id : {} is submitted: {}",
            &fp.id, compliance_res.submitted
        );
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
                        cargo_weight_grams: vec![],
                        weather_conditions: None,
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
                        flight_distance_meters: 0,
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
    use serial_test::serial;
    use test_utils::*;

    async fn run_query_flight(
        storage_client_wrapper: &(dyn StorageClientWrapperTrait + Send + Sync),
    ) -> Response<QueryFlightResponse> {
        let edt = Utc
            .with_ymd_and_hms(2022, 10, 25, 11, 0, 0)
            .unwrap()
            .timestamp();
        let lat = Utc
            .with_ymd_and_hms(2022, 10, 25, 12, 15, 0)
            .unwrap()
            .timestamp();
        query_flight(
            Request::new(QueryFlightRequest {
                is_cargo: false,
                persons: None,
                weight_grams: None,
                earliest_departure_time: Some(Timestamp {
                    seconds: edt,
                    nanos: 0,
                }),
                latest_arrival_time: Some(Timestamp {
                    seconds: lat,
                    nanos: 0,
                }),
                vertiport_depart_id: "vertiport1".to_string(),
                vertiport_arrive_id: "vertiport2".to_string(),
            }),
            storage_client_wrapper,
        )
        .await
        .unwrap()
    }

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
    #[serial]
    async fn test_query_flight() {
        init_logger();
        let storage_client_wrapper = create_storage_client_stub();
        init_router(&storage_client_wrapper).await;
        let res = run_query_flight(&storage_client_wrapper).await;
        assert_eq!(res.into_inner().flights.len(), 5);
    }

    #[tokio::test]
    #[serial]
    async fn test_confirm_flight() {
        init_logger();
        let storage_client_wrapper = create_storage_client_stub();
        let compliance_client_wrapper = create_compliance_client_stub();
        init_router(&storage_client_wrapper).await;
        let res = confirm_flight(
            Request::new(Id {
                id: "flight1".to_string(),
            }),
            &storage_client_wrapper,
            &compliance_client_wrapper,
        )
        .await;
        //test confirming a flight that does not exist will return an error
        assert_eq!(res.is_err(), true);
        let qf_res = run_query_flight(&storage_client_wrapper).await;
        let res = confirm_flight(
            Request::new(Id {
                id: qf_res.into_inner().flights[0]
                    .flight_plan
                    .as_ref()
                    .unwrap()
                    .id
                    .to_string(),
            }),
            &storage_client_wrapper,
            &compliance_client_wrapper,
        )
        .await;
        //test confirming a flight that does exist will return a success
        assert_eq!(res.unwrap().into_inner().confirmed, true);
    }

    ///4. destination vertiport is available for about 15 minutes, no other restrictions
    /// - returns 2 flights (assuming 10 minutes needed for unloading, this can fit 2 flights
    /// if first is exactly at the beginning of 15 minute gap and second is exactly after 5 minutes)
    #[tokio::test]
    #[serial]
    async fn test_query_flight_4_dest_vertiport_tight_availability_should_return_one_flight() {
        init_logger();
        let storage_client_wrapper = create_storage_client_stub();
        init_router(&storage_client_wrapper).await;

        let edt = Utc
            .with_ymd_and_hms(2022, 10, 25, 14, 00, 0)
            .unwrap()
            .timestamp();
        let lat = Utc
            .with_ymd_and_hms(2022, 10, 25, 15, 10, 0)
            .unwrap()
            .timestamp();
        let res = query_flight(
            Request::new(QueryFlightRequest {
                is_cargo: false,
                persons: None,
                weight_grams: None,
                earliest_departure_time: Some(Timestamp {
                    seconds: edt,
                    nanos: 0,
                }),
                latest_arrival_time: Some(Timestamp {
                    seconds: lat,
                    nanos: 0,
                }),
                vertiport_depart_id: "vertiport1".to_string(),
                vertiport_arrive_id: "vertiport2".to_string(),
            }),
            &storage_client_wrapper,
        )
        .await
        .unwrap();
        assert_eq!(res.into_inner().flights.len(), 2);
    }
}
