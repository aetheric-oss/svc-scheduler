//! Implementation of the queries/actions that the scheduler service can perform.

use crate::grpc::server::grpc_server::{
    CancelItineraryResponse, ConfirmItineraryRequest, ConfirmItineraryResponse, FlightPriority,
    FlightStatus, Id, Itinerary, QueryFlightPlan, QueryFlightRequest, QueryFlightResponse,
};
use crate::router::router_utils::router_state::get_possible_flights;
use crate::router::router_utils::router_state::{
    init_router_from_vertiports, is_router_initialized,
};

use svc_compliance_client_grpc::client::FlightPlanRequest;
use svc_storage_client_grpc::{
    resources::{
        flight_plan::{self, Data as FlightPlanData},
        itinerary,
    },
    AdvancedSearchFilter, Id as StorageId, IdList,
};

use crate::grpc::client::{ComplianceClientWrapperTrait, StorageClientWrapperTrait};

use log::{debug, error, info, warn};
use once_cell::sync::OnceCell;
use prost_types::{FieldMask, Timestamp};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;
use tonic::{Request, Response, Status};
use uuid::Uuid;

const ITINERARY_EXPIRATION_S: u64 = 30;

// Some itineraries might be intermixed with flight plans
//  that already exist (rideshare)
#[derive(PartialEq, Clone)]
enum FlightPlanType {
    Draft,
    Existing,
}

/// gets or creates a new hashmap of unconfirmed flight plans
fn unconfirmed_flight_plans() -> &'static Mutex<HashMap<String, FlightPlanData>> {
    static INSTANCE: OnceCell<Mutex<HashMap<String, FlightPlanData>>> = OnceCell::new();
    INSTANCE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Clone)]
struct ItineraryFlightPlan {
    fp_type: FlightPlanType,
    fp_id: String,
}

/// gets or creates a new hashmap of unconfirmed itineraries
/// "itineraries" are a list of flight plan IDs, which can represent DRAFT
///  (in memory) or EXISTING (in database) flight plans
fn unconfirmed_itineraries() -> &'static Mutex<HashMap<String, Vec<ItineraryFlightPlan>>> {
    static INSTANCE: OnceCell<Mutex<HashMap<String, Vec<ItineraryFlightPlan>>>> = OnceCell::new();
    INSTANCE.get_or_init(|| Mutex::new(HashMap::new()))
}

//-------------------------------------------------------------------
// Helper Functions
//-------------------------------------------------------------------

fn create_scheduler_fp_from_storage_fp(fp_id: String, fp: &FlightPlanData) -> QueryFlightPlan {
    QueryFlightPlan {
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
    }
}

/// spawns a thread that will cancel the itinerary after a certain amount of time (ITINERARY_EXPIRATION_S)
fn cancel_itinerary_after_timeout(id: String) {
    tokio::spawn(async move {
        tokio::time::sleep(core::time::Duration::from_secs(ITINERARY_EXPIRATION_S)).await;
        remove_draft_itinerary_by_id(&id);
        debug!("Flight plan {} was not confirmed in time, cancelling", id);
    });
}

/// Gets itinerary from hash map of unconfirmed itineraries
fn get_draft_itinerary_by_id(id: &str) -> Option<Vec<ItineraryFlightPlan>> {
    unconfirmed_itineraries()
        .lock()
        .expect("Mutex Lock Error getting itinerary from temp storage")
        .get(id)
        .cloned()
}

/// Gets flight plan from hash map of unconfirmed flight plans
fn get_draft_fp_by_id(id: &str) -> Option<FlightPlanData> {
    unconfirmed_flight_plans()
        .lock()
        .expect("Mutex Lock Error getting flight plan from temp storage")
        .get(id)
        .cloned()
}

/// Removes flight plan from hash map of unconfirmed flight plans
fn remove_draft_fp_by_id(id: &str) -> bool {
    let mut flight_plans = unconfirmed_flight_plans()
        .lock()
        .expect("(remove_draft_fp_by_id) mutex Lock Error removing flight plan from temp storage");

    match flight_plans.remove(id) {
        Some(_) => {
            debug!(
                "(remove_draft_fp_by_id) with id {} removed from local cache",
                &id
            );
            true
        }
        _ => {
            debug!(
                "(remove_draft_fp_by_id) no such flight plan with ID {} in cache",
                &id
            );
            false
        }
    }
}

/// Removes itinerary from hash map of unconfirmed flight plans
fn remove_draft_itinerary_by_id(id: &str) -> bool {
    let mut itineraries = unconfirmed_itineraries().lock().expect(
        "(remove_draft_itinerary_by_id) mutex Lock Error removing itinerary from temp storage",
    );

    let Some(itinerary) = itineraries.get(id) else {
        debug!("(remove_draft_itinerary_by_id) no such itinerary with ID {} in cache", &id);
        return false;
    };

    // Remove draft flight plans associated with this itinerary
    for fp in itinerary {
        if fp.fp_type == FlightPlanType::Draft {
            // Ignore if not found
            let _ = remove_draft_fp_by_id(&fp.fp_id);
        }
    }

    itineraries.remove(id);

    info!("cancel_itinerary with id {} removed from local cache", &id);
    true
}

/// Confirms a flight plan
async fn confirm_draft_flight_plan(
    flight_plan_id: String,
    storage_client_wrapper: &(dyn StorageClientWrapperTrait + Send + Sync),
    compliance_client_wrapper: &(dyn ComplianceClientWrapperTrait + Send + Sync),
) -> Result<flight_plan::Object, Status> {
    let Some(flight_plan) = get_draft_fp_by_id(&flight_plan_id) else {
        return Err(Status::internal("Draft flight plan ID doesn't exist."));
    };

    //
    // Confirm a flight plan with the database
    //
    let Some(fp) = storage_client_wrapper
        .insert_flight_plan(Request::new(flight_plan))
        .await?
        .into_inner()
        .object
    else {
        return Err(Status::internal("Failed to add a flight plan to the database."));
    };

    info!(
        "(confirm_itinerary) flight plan with draft id: {} 
        inserted into storage with permanent id: {}",
        &flight_plan_id, &fp.id
    );

    //
    // Remove the flight plan from the cache now that it's in the database
    //
    let _ = remove_draft_fp_by_id(&flight_plan_id);

    // TODO R3
    // Retrieve user data in case that's relevant to compliance

    let compliance_res = compliance_client_wrapper
        .submit_flight_plan(Request::new(FlightPlanRequest {
            flight_plan_id: fp.id.clone(),
            data: "".to_string(),
        }))
        .await?
        .into_inner();
    info!(
        "confirm_draft_flight_plan: Compliance response for flight plan id : {} is submitted: {}",
        &fp.id, compliance_res.submitted
    );

    Ok(fp)
}

/// Registers an itinerary with svc-storage.
///
/// There's two steps involved with registering an itinerary:
/// 1) Register a new itinerary with the `itinerary` table in the database
/// 2) Link flight plan IDs to the itinerary (a separate `itinerary_flight_plan` table)
async fn create_itinerary(
    draft_itinerary_id: String,
    user_id: String,
    confirmed_flight_plan_ids: Vec<String>,
    storage_client_wrapper: &(dyn StorageClientWrapperTrait + Send + Sync),
) -> Result<itinerary::Object, Status> {
    //
    // 1) Add itinerary to `itinerary` DB table
    //
    let data = itinerary::Data {
        user_id: user_id.to_string(),
        status: itinerary::ItineraryStatus::Active as i32,
    };

    let Some(db_itinerary) = storage_client_wrapper
        .insert_itinerary(Request::new(data))
        .await?
        .into_inner()
        .object
    else {
        return Err(Status::internal("Couldn't add itinerary to storage."));
    };

    //
    // 2) Link flight plans to itinerary in `itinerary_flight_plan`
    //
    let request = tonic::Request::new(itinerary::ItineraryFlightPlans {
        id: db_itinerary.id.clone(),
        other_id_list: Some(IdList {
            ids: confirmed_flight_plan_ids,
        }),
    });

    let Ok(_) = storage_client_wrapper
        .link_flight_plan(request)
        .await
    else {
        return Err(Status::internal("Failed to link flight plans to the itinerary."));
    };

    // At this point all draft flight plans have been confirmed and can be
    //  removed from local memory along with the draft itinerary
    let _ = remove_draft_itinerary_by_id(&draft_itinerary_id);
    Ok(db_itinerary)
}

//-------------------------------------------------------------------
// API Functions
//-------------------------------------------------------------------

/// Initializes the router from vertiports in the database.
/// TODO(R3): The routing may be moved to a SQL Graph Database.
///          This function will be removed in that case.
pub async fn init_router(storage_client_wrapper: &(dyn StorageClientWrapperTrait + Send + Sync)) {
    let result = storage_client_wrapper
        .vertiports(Request::new(AdvancedSearchFilter {
            filters: vec![],
            page_number: 0,
            results_per_page: 50,
            order_by: vec![],
        }))
        .await;

    let Ok(vertiports) = result else {
        let error_msg = "Failed to get vertiports from storage service".to_string();
        debug!("{}: {:?}", error_msg, result.unwrap_err());
        panic!("{}", error_msg);
    };

    let vertiports = vertiports.into_inner().list;
    info!("Initializing router with {} vertiports ", vertiports.len());
    if !is_router_initialized() {
        let res = init_router_from_vertiports(&vertiports);
        if res.is_err() {
            error!("Failed to initialize router: {}", res.err().unwrap());
        }
    }
}

/// Finds the first possible flight for customer location, flight type and requested time.
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
    let depart_vertipads = storage_client_wrapper
        .vertipads(Request::new(
            AdvancedSearchFilter::search_equals(
                "vertiport_id".to_owned(),
                flight_request.vertiport_depart_id.clone(),
            )
            .and_is_null("deleted_at".to_owned()),
        ))
        .await?
        .into_inner()
        .list;
    let arrive_vertipads = storage_client_wrapper
        .vertipads(Request::new(
            AdvancedSearchFilter::search_equals(
                "vertiport_id".to_owned(),
                flight_request.vertiport_arrive_id.clone(),
            )
            .and_is_null("deleted_at".to_owned()),
        ))
        .await?
        .into_inner()
        .list;
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
    //- this assumes that all landed flights have updated vehicle.last_vertiport_id (otherwise we would need to look in to the past)
    //Plans are used by lib_router to find aircraft and vertiport availability and aircraft predicted location
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
            .and_is_null("deleted_at".to_owned()),
        ))
        .await?
        .into_inner()
        .list;
    //4. get all possible flight plans from router
    let Ok(flight_plans) = get_possible_flights(
        depart_vertiport,
        arrive_vertiport,
        depart_vertipads,
        arrive_vertipads,
        flight_request.earliest_departure_time,
        flight_request.latest_arrival_time,
        aircrafts,
        existing_flight_plans,
    ) else {
        return Err(Status::internal(
            "Routing failed; No flight plans available."
        ));
    };

    if flight_plans.is_empty() {
        return Err(Status::not_found("No flight plans available.".to_string()));
    }

    info!("Found {} flight plans from router", &flight_plans.len());

    //5. create draft itinerary and flight plans (in memory)
    let mut itineraries: Vec<Itinerary> = vec![];
    for (fp, deadhead_fps) in &flight_plans {
        let fp_id = Uuid::new_v4().to_string();
        info!("Adding draft flight plan with temporary id: {}", &fp_id);
        unconfirmed_flight_plans()
            .lock()
            .expect("Mutex Lock Error inserting flight plan into temp storage")
            .insert(fp_id.clone(), fp.clone());

        let item = create_scheduler_fp_from_storage_fp(fp_id.clone(), fp);
        let deadhead_flight_plans: Vec<QueryFlightPlan> = deadhead_fps
            .iter()
            .map(|fp| create_scheduler_fp_from_storage_fp(Uuid::new_v4().to_string(), fp))
            .collect();
        debug!("flight plan: {:?}", &item);

        //
        // Create Itinerary
        //
        // TODO R3 account for existing flight plans combined with draft flight plans
        let mut total_fps = vec![];
        total_fps.push(ItineraryFlightPlan {
            fp_type: FlightPlanType::Draft,
            fp_id: item.id.clone(),
        });

        for deadhead_flight_plan in &deadhead_flight_plans {
            total_fps.push(ItineraryFlightPlan {
                fp_type: FlightPlanType::Draft,
                fp_id: deadhead_flight_plan.id.clone(),
            });
        }

        let itinerary_id = Uuid::new_v4().to_string();
        unconfirmed_itineraries()
            .lock()
            .expect("Mutex Lock Error inserting flight plan into temp storage")
            .insert(itinerary_id.clone(), total_fps);
        cancel_itinerary_after_timeout(itinerary_id.clone());

        itineraries.push(Itinerary {
            id: itinerary_id,
            flight_plan: Some(item),
            deadhead_flight_plans,
        });
    }

    //6. return response
    let response = QueryFlightResponse { itineraries };
    info!(
        "query_flight returning: {} flight plans",
        &response.itineraries.len()
    );
    Ok(Response::new(response))
}

/// Confirms an itinerary
pub async fn confirm_itinerary(
    request: Request<ConfirmItineraryRequest>,
    storage_client_wrapper: &(dyn StorageClientWrapperTrait + Send + Sync),
    compliance_client_wrapper: &(dyn ComplianceClientWrapperTrait + Send + Sync),
) -> Result<Response<ConfirmItineraryResponse>, Status> {
    //
    // Return if the itinerary has expired in cache
    //
    let request = request.into_inner();
    let draft_itinerary_id = request.id;
    info!("(confirm_itinerary) with id {}", &draft_itinerary_id);

    let Some(draft_itinerary_flights) = get_draft_itinerary_by_id(&draft_itinerary_id) else {
        return Err(Status::not_found("Itinerary ID not found or timed out."));
    };

    //
    // For each Draft flight in the itinerary, push to svc-storage
    //
    let mut confirmed_flight_plan_ids: Vec<String> = vec![];
    for fp in draft_itinerary_flights {
        if fp.fp_type == FlightPlanType::Existing {
            // TODO R3 update svc-storage flight plan with new passenger count
            // let data = flight_plan::UpdateObject { ... }

            confirmed_flight_plan_ids.push(fp.fp_id);
            continue;
        }

        let confirmation =
            confirm_draft_flight_plan(fp.fp_id, storage_client_wrapper, compliance_client_wrapper)
                .await;

        let Ok(confirmed_fp) = confirmation else {
            break;
        };

        confirmed_flight_plan_ids.push(confirmed_fp.id);
    }

    //
    // Create and insert itinerary
    //
    let result = create_itinerary(
        draft_itinerary_id,
        request.user_id,
        confirmed_flight_plan_ids,
        storage_client_wrapper,
    )
    .await;

    match result {
        Ok(itinerary) => {
            let response = ConfirmItineraryResponse {
                id: itinerary.id,
                confirmed: true,
                confirmation_time: Some(Timestamp::from(SystemTime::now())),
            };

            Ok(Response::new(response))
        }
        Err(e) => Err(e),
    }
}

/// Cancels a draft or confirmed flight plan
pub async fn cancel_itinerary(
    request: Request<Id>,
    storage_client_wrapper: &(dyn StorageClientWrapperTrait + Send + Sync),
) -> Result<Response<CancelItineraryResponse>, Status> {
    let itinerary_id = request.into_inner().id;
    info!("(cancel_itinerary) for id {}", &itinerary_id);

    //
    // Look within unconfirmed itineraries
    //
    if remove_draft_itinerary_by_id(&itinerary_id) {
        let sys_time = SystemTime::now();
        let response = CancelItineraryResponse {
            id: itinerary_id,
            cancelled: true,
            cancellation_time: Some(Timestamp::from(sys_time)),
            reason: "user cancelled".into(),
        };
        return Ok(Response::new(response));
    }

    //
    // Look within confirmed itineraries
    //
    let Ok(_) = storage_client_wrapper
        .itinerary_by_id(Request::new(StorageId { id: itinerary_id.clone() }))
        .await
    else {
        let err_msg = format!(
            "(cancel_itinerary) id {} not found in local cache nor storage",
            &itinerary_id
        );
        return Err(Status::not_found(err_msg));
    };

    //
    // TODO R3 Don't allow cancellations within X minutes of the first flight
    //

    //
    // Remove itinerary
    //
    let update_object = itinerary::UpdateObject {
        id: itinerary_id.clone(),
        data: Option::from(itinerary::Data {
            user_id: "".to_string(), // will be masked
            status: itinerary::ItineraryStatus::Cancelled as i32,
        }),
        mask: Some(FieldMask {
            paths: vec!["status".to_string()],
        }),
    };
    let Ok(_) = storage_client_wrapper
        .update_itinerary(Request::new(update_object))
        .await
    else {
        return Err(Status::internal("Unable to cancel itinerary."));
    };

    info!(
        "cancel_itinerary with id {} cancelled in storage",
        &itinerary_id
    );

    let Ok(response) = storage_client_wrapper
        .get_itinerary_flight_plan_ids(Request::new(StorageId { id: itinerary_id.clone() }))
        .await
    else {
        return Err(Status::internal("Unable to get linked flight plans"));
    };

    //
    // Cancel associated flight plans
    //
    let mut flight_plan = flight_plan::Data::default();
    flight_plan.flight_status = flight_plan::FlightStatus::Cancelled as i32;
    for id in response.into_inner().ids {
        //
        // TODO Don't cancel flight plan if it exists in another itinerary
        //
        let request = flight_plan::UpdateObject {
            id: id.clone(),
            data: Option::from(flight_plan.clone()),
            mask: Some(FieldMask {
                paths: vec!["flight_status".to_string()],
            }),
        };
        let result = storage_client_wrapper
            .update_flight_plan(Request::new(request))
            .await;

        // Keep going even if there's a warning
        if result.is_err() {
            warn!("WARNING: Could not cancel flight plan with ID: {}", id);
        }
    }

    //
    // Reply
    //
    let sys_time = SystemTime::now();
    let response = CancelItineraryResponse {
        id: itinerary_id,
        cancelled: true,
        cancellation_time: Some(Timestamp::from(sys_time)),
        reason: "user cancelled".into(),
    };
    Ok(Response::new(response))
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
            .with_ymd_and_hms(2022, 10, 25, 11, 20, 0)
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
        test_utils::init_router(&storage_client_wrapper).await;
        let res = run_query_flight(&storage_client_wrapper).await;
        assert_eq!(res.into_inner().itineraries.len(), 5);
    }

    #[tokio::test]
    #[serial]
    async fn test_confirm_itinerary() {
        init_logger();
        let storage_client_wrapper = create_storage_client_stub();
        let compliance_client_wrapper = create_compliance_client_stub();
        test_utils::init_router(&storage_client_wrapper).await;
        let res = confirm_itinerary(
            Request::new(ConfirmItineraryRequest {
                id: "itinerary1".to_string(),
                user_id: "".to_string(),
            }),
            &storage_client_wrapper,
            &compliance_client_wrapper,
        )
        .await;
        //test confirming a flight that does not exist will return an error
        assert_eq!(res.is_err(), true);
        let qf_res = run_query_flight(&storage_client_wrapper).await;
        let res = confirm_itinerary(
            Request::new(ConfirmItineraryRequest {
                id: qf_res.into_inner().itineraries[0].id.clone(),
                user_id: "".to_string(),
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
    async fn test_query_flight_4_dest_vertiport_tight_availability_should_return_two_flights() {
        init_logger();
        let storage_client_wrapper = create_storage_client_stub();
        test_utils::init_router(&storage_client_wrapper).await;

        let edt = Utc
            .with_ymd_and_hms(2022, 10, 25, 14, 20, 0)
            .unwrap()
            .timestamp();
        let lat = Utc
            .with_ymd_and_hms(2022, 10, 25, 15, 15, 0)
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
                vertiport_depart_id: "vertiport3".to_string(),
                vertiport_arrive_id: "vertiport2".to_string(),
            }),
            &storage_client_wrapper,
        )
        .await
        .unwrap();
        assert_eq!(res.into_inner().itineraries.len(), 2);
    }

    ///5. source or destination vertiport doesn't have any vertipad free for the time range
    ///no flight plans returned
    #[tokio::test]
    #[serial]
    async fn test_query_flight_5_dest_vertiport_no_availability_should_return_zero_flights() {
        init_logger();
        let storage_client_wrapper = create_storage_client_stub();
        test_utils::init_router(&storage_client_wrapper).await;

        let edt = Utc
            .with_ymd_and_hms(2022, 10, 26, 14, 00, 0)
            .unwrap()
            .timestamp();
        let lat = Utc
            .with_ymd_and_hms(2022, 10, 26, 14, 40, 0)
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
                vertiport_depart_id: "vertiport2".to_string(),
                vertiport_arrive_id: "vertiport1".to_string(),
            }),
            &storage_client_wrapper,
        )
        .await;
        assert_eq!(
            res.unwrap_err()
                .message()
                .contains("No flight plans available"),
            true
        );
    }

    ///6. vertiports are available but aircrafts are not at the vertiport for the requested time
    /// but at least one aircraft is IN FLIGHT to requested vertiport for that time and has availability for a next flight.
    /// 	- skips all unavailable time slots (4) and returns only time slots from when aircraft is available (1)
    #[tokio::test]
    #[serial]
    async fn test_query_flight_6_no_aircraft_at_vertiport() {
        init_logger();
        let storage_client_wrapper = create_storage_client_stub();
        test_utils::init_router(&storage_client_wrapper).await;
        let edt = Utc
            .with_ymd_and_hms(2022, 10, 26, 14, 15, 0)
            .unwrap()
            .timestamp();
        let lat = Utc
            .with_ymd_and_hms(2022, 10, 26, 15, 00, 0)
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
                vertiport_arrive_id: "vertiport3".to_string(),
            }),
            &storage_client_wrapper,
        )
        .await
        .unwrap()
        .into_inner();

        assert_eq!(res.itineraries.len(), 1);
        assert_eq!(res.itineraries[0].deadhead_flight_plans.len(), 0);
    }

    /// 7. vertiports are available but aircrafts are not at the vertiport for the requested time
    /// but at least one aircraft is PARKED at other vertiport for the "requested time - N minutes"
    #[tokio::test]
    #[serial]
    async fn test_query_flight_7_deadhead_flight_of_parked_vehicle() {
        init_logger();
        let storage_client_wrapper = create_storage_client_stub();
        test_utils::init_router(&storage_client_wrapper).await;
        let res = query_flight(
            Request::new(QueryFlightRequest {
                is_cargo: false,
                persons: None,
                weight_grams: None,
                earliest_departure_time: Some(get_timestamp_from_utc_date("2022-10-26 16:00:00")),
                latest_arrival_time: Some(get_timestamp_from_utc_date("2022-10-26 16:30:00")),
                vertiport_depart_id: "vertiport3".to_string(),
                vertiport_arrive_id: "vertiport1".to_string(),
            }),
            &storage_client_wrapper,
        )
        .await
        .unwrap()
        .into_inner();

        assert_eq!(res.itineraries.len(), 1);
        assert_eq!(res.itineraries[0].deadhead_flight_plans.len(), 1);
    }

    /// 8. vertiports are available but aircrafts are not at the vertiport for the requested time
    /// but at least one aircraft is EN ROUTE to other vertiport for the "requested time - N minutes - M minutes"
    #[tokio::test]
    #[serial]
    async fn test_query_flight_8_deadhead_flight_of_in_flight_vehicle() {
        init_logger();
        let storage_client_wrapper = create_storage_client_stub();
        test_utils::init_router(&storage_client_wrapper).await;
        let res = query_flight(
            Request::new(QueryFlightRequest {
                is_cargo: false,
                persons: None,
                weight_grams: None,
                earliest_departure_time: Some(get_timestamp_from_utc_date("2022-10-27 12:30:00")),
                latest_arrival_time: Some(get_timestamp_from_utc_date("2022-10-27 13:30:00")),
                vertiport_depart_id: "vertiport2".to_string(),
                vertiport_arrive_id: "vertiport1".to_string(),
            }),
            &storage_client_wrapper,
        )
        .await
        .unwrap()
        .into_inner();
        assert_eq!(res.itineraries.len(), 1);
        assert_eq!(res.itineraries[0].deadhead_flight_plans.len(), 1);
    }

    /// 9. destination vertiport is not available because of capacity
    /// - if at requested time all pads are occupied and at least one is parked (not loading/unloading),
    /// extra flight plan should be created to move idle aircraft to the nearest unoccupied vertiport (or to preferred vertiport in hub and spoke model)
    #[tokio::test]
    #[serial]
    async fn test_query_flight_9_deadhead_destination_flight_no_capacity_at_destination_vertiport()
    {
        init_logger();
        let storage_client_wrapper = create_storage_client_stub();
        test_utils::init_router(&storage_client_wrapper).await;
        let res = query_flight(
            Request::new(QueryFlightRequest {
                is_cargo: false,
                persons: None,
                weight_grams: None,
                earliest_departure_time: Some(get_timestamp_from_utc_date("2022-10-27 15:10:00")),
                latest_arrival_time: Some(get_timestamp_from_utc_date("2022-10-27 16:00:00")),
                vertiport_depart_id: "vertiport2".to_string(),
                vertiport_arrive_id: "vertiport4".to_string(),
            }),
            &storage_client_wrapper,
        )
        .await
        .unwrap()
        .into_inner();
        assert_eq!(res.itineraries.len(), 1);
        assert_eq!(res.itineraries[0].deadhead_flight_plans.len(), 1);
    }
}
