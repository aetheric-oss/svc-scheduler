//! Implementation of the queries/actions that the scheduler service can perform.

use crate::grpc::server::grpc_server::{
    CancelItineraryResponse, ConfirmItineraryRequest, ConfirmItineraryResponse, Id, Itinerary,
    QueryFlightRequest, QueryFlightResponse,
};
use crate::router::router_utils::router_state::get_possible_flights;
use crate::router::router_utils::router_state::{
    init_router_from_vertiports, is_router_initialized,
};

use crate::grpc::client::get_clients;
use svc_compliance_client_grpc::client::FlightPlanRequest;
use svc_compliance_client_grpc::service::Client as ComplianceServiceClient;
use svc_storage_client_grpc::prelude::{Id as StorageId, *};

use lazy_static::lazy_static;
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

lazy_static! {
    static ref UNCONFIRMED_FLIGHT_PLANS: Mutex<HashMap<String, flight_plan::Data>> =
        Mutex::new(HashMap::new());
    static ref UNCONFIRMED_ITINERARIES: Mutex<HashMap<String, Vec<ItineraryFlightPlan>>> =
        Mutex::new(HashMap::new());
}
/// gets unconfirmed flight plans
fn unconfirmed_flight_plans() -> &'static Mutex<HashMap<String, flight_plan::Data>> {
    &UNCONFIRMED_FLIGHT_PLANS
}
/// gets a hashmap of unconfirmed itineraries
/// "itineraries" are a list of flight plan IDs, which can represent DRAFT
///  (in memory) or EXISTING (in database) flight plans
fn unconfirmed_itineraries() -> &'static Mutex<HashMap<String, Vec<ItineraryFlightPlan>>> {
    &UNCONFIRMED_ITINERARIES
}

#[derive(Clone)]
struct ItineraryFlightPlan {
    fp_type: FlightPlanType,
    fp_id: String,
}

//-------------------------------------------------------------------
// Helper Functions
//-------------------------------------------------------------------

fn fp_from_storage(fp_id: String, fp: flight_plan::Data) -> Result<flight_plan::Object, ()> {
    if fp.departure_vertiport_id.is_none() {
        grpc_error!(
            "(fp_from_storage) flight plan {} has no departure vertiport. Should not be possible.",
            fp_id
        );
        return Err(());
    }

    if fp.destination_vertiport_id.is_none() {
        grpc_error!(
            "(fp_from_storage) flight plan {} has no destination vertiport",
            fp_id
        );
        return Err(());
    }

    Ok(flight_plan::Object {
        id: fp_id,
        data: Some(fp),
    })
}

/// spawns a thread that will cancel the itinerary after a certain amount of time (ITINERARY_EXPIRATION_S)
fn cancel_itinerary_after_timeout(id: String) {
    tokio::spawn(async move {
        tokio::time::sleep(core::time::Duration::from_secs(ITINERARY_EXPIRATION_S)).await;
        remove_draft_itinerary_by_id(&id);
        grpc_debug!("Flight plan {} was not confirmed in time, cancelling", id);
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
fn get_draft_fp_by_id(id: &str) -> Option<flight_plan::Data> {
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
            grpc_debug!(
                "(remove_draft_fp_by_id) with id {} removed from local cache",
                &id
            );
            true
        }
        _ => {
            grpc_debug!(
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
        grpc_debug!("(remove_draft_itinerary_by_id) no such itinerary with ID {} in cache", &id);
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

    grpc_info!("cancel_itinerary with id {} removed from local cache", &id);
    true
}

/// Confirms a flight plan by registering it with svc-storage
/// After confirmation, the flight plan will be removed from local cache
async fn confirm_draft_flight_plan(flight_plan_id: String) -> Result<flight_plan::Object, Status> {
    let clients = get_clients().await;

    let Some(flight_plan) = get_draft_fp_by_id(&flight_plan_id) else {
        return Err(Status::internal("Draft flight plan ID doesn't exist."));
    };

    //
    // Confirm a flight plan with the database
    //
    let Some(fp) = clients.storage.flight_plan
        .insert(flight_plan)
        .await?
        .into_inner()
        .object
    else {
        return Err(Status::internal("Failed to add a flight plan to the database."));
    };

    grpc_info!(
        "(confirm_draft_flight_plan) flight plan with draft id: {} 
        inserted into storage with permanent id: {}",
        &flight_plan_id,
        &fp.id
    );

    //
    // Remove the flight plan from the cache now that it's in the database
    //
    let _ = remove_draft_fp_by_id(&flight_plan_id);

    // TODO(R3)
    // Retrieve user data in case that's relevant to compliance
    let compliance_res = clients
        .compliance
        .submit_flight_plan(FlightPlanRequest {
            flight_plan_id: fp.id.clone(),
            data: "".to_string(),
        })
        .await?
        .into_inner();
    grpc_info!(
        "(confirm_draft_flight_plan) Compliance response for flight plan id : {} is submitted: {}",
        &fp.id,
        compliance_res.submitted
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
) -> Result<itinerary::Object, Status> {
    let clients = get_clients().await;
    //
    // 1) Add itinerary to `itinerary` DB table
    //
    let data = itinerary::Data {
        user_id: user_id.to_string(),
        status: itinerary::ItineraryStatus::Active as i32,
    };

    let Some(db_itinerary) = clients.storage.itinerary
        .insert(data)
        .await?
        .into_inner()
        .object
    else {
        return Err(Status::internal("Couldn't add itinerary to storage."));
    };

    //
    // 2) Link flight plans to itinerary in `itinerary_flight_plan`
    //
    clients
        .storage
        .itinerary_flight_plan_link
        .link(itinerary::ItineraryFlightPlans {
            id: db_itinerary.id.clone(),
            other_id_list: Some(IdList {
                ids: confirmed_flight_plan_ids,
            }),
        })
        .await?;

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
pub async fn init_router() {
    let clients = get_clients().await;
    let result = clients
        .storage
        .vertiport
        .search(AdvancedSearchFilter {
            filters: vec![],
            page_number: 0,
            results_per_page: 50,
            order_by: vec![],
        })
        .await;

    let vertiports = match result {
        Ok(vertiports) => vertiports,
        Err(e) => {
            let error_msg = format!("Failed to get vertiports from storage service: {}", e);
            grpc_debug!("{}", error_msg);
            let result = clients
                .storage
                .vertiport
                .search(AdvancedSearchFilter {
                    filters: vec![],
                    page_number: 0,
                    results_per_page: 50,
                    order_by: vec![],
                })
                .await;
            let Ok(vertiports) = result else {
                panic!("Could not get vertiports, retry failed.")
            };
            vertiports
        } //panic!("{}", error_msg);
    };

    let vertiports = vertiports.into_inner().list;
    grpc_info!("Initializing router with {} vertiports ", vertiports.len());
    if !is_router_initialized() {
        let res = init_router_from_vertiports(&vertiports).await;
        if let Err(res) = res {
            grpc_error!("Failed to initialize router: {}", res);
        }
    }
}

/// Finds the first possible flight for customer location, flight type and requested time.
pub async fn query_flight(
    request: Request<QueryFlightRequest>,
) -> Result<Response<QueryFlightResponse>, Status> {
    let clients = get_clients().await;
    let flight_request = request.into_inner();
    // 1. get vertiports
    grpc_info!(
        "(query_flight) with vertiport depart, arrive ids: {}, {}",
        &flight_request.vertiport_depart_id,
        &flight_request.vertiport_arrive_id
    );
    let depart_vertiport = clients
        .storage
        .vertiport
        .get_by_id(StorageId {
            id: flight_request.vertiport_depart_id.clone(),
        })
        .await?
        .into_inner();
    let arrive_vertiport = clients
        .storage
        .vertiport
        .get_by_id(StorageId {
            id: flight_request.vertiport_arrive_id.clone(),
        })
        .await?
        .into_inner();
    grpc_debug!(
        "(query_flight) depart_vertiport: {:?}, arrive_vertiport: {:?}",
        &depart_vertiport,
        &arrive_vertiport
    );
    let depart_vertipads = clients
        .storage
        .vertipad
        .search(
            AdvancedSearchFilter::search_equals(
                "vertiport_id".to_owned(),
                flight_request.vertiport_depart_id.clone(),
            )
            .and_is_null("deleted_at".to_owned()),
        )
        .await?
        .into_inner()
        .list;
    grpc_debug!("(query_flight) depart_vertipads: {:?}", depart_vertipads);

    let arrive_vertipads = clients
        .storage
        .vertipad
        .search(
            AdvancedSearchFilter::search_equals(
                "vertiport_id".to_owned(),
                flight_request.vertiport_arrive_id.clone(),
            )
            .and_is_null("deleted_at".to_owned()),
        )
        .await?
        .into_inner()
        .list;
    grpc_debug!("(query_flight) arrive_vertipads: {:?}", arrive_vertipads);

    //2. get all aircraft
    let aircraft = clients
        .storage
        .vehicle
        .search(AdvancedSearchFilter {
            filters: vec![],
            page_number: 0,
            results_per_page: 0,
            order_by: vec![],
        })
        .await?
        .into_inner()
        .list;

    grpc_debug!("(query_flight) found vehicles: {:?}", aircraft);

    //3. get all flight plans from this time to latest departure time (including partially fitting flight plans)
    //- this assumes that all landed flights have updated vehicle.last_vertiport_id (otherwise we would need to look in to the past)
    //Plans are used by lib_router to find aircraft and vertiport availability and aircraft predicted location
    let Some(latest_arrival_time) = flight_request.latest_arrival_time.clone() else {
        grpc_warn!("(query_flight) latest arrival time not provided.");
        return Err(Status::invalid_argument("Routing failed; latest arrival time not provided."));
    };
    let existing_flight_plans = query_flight_plans_for_latest_arrival(latest_arrival_time).await?;
    grpc_debug!(
        "(query_flight) found existing flight plans: {:?}",
        existing_flight_plans
    );

    //4. get all possible flight plans from router
    let Ok(flight_plans) = get_possible_flights(
        depart_vertiport,
        arrive_vertiport,
        depart_vertipads,
        arrive_vertipads,
        flight_request.earliest_departure_time,
        flight_request.latest_arrival_time,
        aircraft,
        existing_flight_plans,
    ).await else {
        let error = String::from("Routing failed; No flight plans available.");
        grpc_error!("(query_flight) {}", error);
        return Err(Status::internal(error));
    };

    if flight_plans.is_empty() {
        let error = String::from("No flight plans available.");
        grpc_info!("(query_flight) {}", error);
        return Err(Status::not_found(error));
    }

    grpc_info!(
        "(query_flight) Found {} flight plans from router",
        &flight_plans.len()
    );

    //5. create draft itinerary and flight plans (in memory)
    let mut itineraries: Vec<Itinerary> = vec![];
    for (fp, deadhead_fps) in &flight_plans {
        let fp_id = Uuid::new_v4().to_string();
        let Ok(item) = fp_from_storage(fp_id.clone(), fp.clone()) else {
            grpc_warn!("(query_flight) invalid flight plan ({:?}), skipping.", fp_id);
            continue;
        };

        grpc_info!(
            "(query_flight) Adding draft flight plan with temporary id: {}",
            &fp_id
        );
        unconfirmed_flight_plans()
            .lock()
            .expect("Mutex Lock Error inserting flight plan into temp storage")
            .insert(fp_id.clone(), fp.clone());

        let deadhead_flight_plans: Vec<flight_plan::Object> = deadhead_fps
            .iter()
            .filter_map(|fp| fp_from_storage(Uuid::new_v4().to_string(), fp.clone()).ok())
            .collect();
        grpc_debug!("(query_flight) flight plan: {:?}", &item);

        //
        // Create Itinerary
        //
        // TODO(R3) account for existing flight plans combined with draft flight plans
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
    grpc_info!(
        "(query_flight) query_flight returning: {} flight plans",
        &response.itineraries.len()
    );
    Ok(Response::new(response))
}

/// Get all flight plans from current time to latest departure time (including partially fitting flight plans)
pub async fn query_flight_plans_for_latest_arrival(
    latest_arrival_time: Timestamp,
) -> Result<Vec<flight_plan::Object>, Status> {
    let clients = get_clients().await;

    Ok(clients
        .storage
        .flight_plan
        .search(
            AdvancedSearchFilter::search_less_or_equal(
                "scheduled_arrival".to_owned(),
                latest_arrival_time.to_string(),
            )
            .and_is_null("deleted_at".to_owned())
            .and_not_in(
                "flight_status".to_owned(),
                vec![
                    (flight_plan::FlightStatus::Finished as i32).to_string(),
                    (flight_plan::FlightStatus::Cancelled as i32).to_string(),
                ],
            ),
        )
        .await?
        .into_inner()
        .list)
}

/// Confirms an itinerary
pub async fn confirm_itinerary(
    request: Request<ConfirmItineraryRequest>,
) -> Result<Response<ConfirmItineraryResponse>, Status> {
    //
    // Return if the itinerary has expired in cache
    //
    let request = request.into_inner();
    let draft_itinerary_id = request.id;
    grpc_info!("(confirm_itinerary) with id {}", &draft_itinerary_id);

    let Some(draft_itinerary_flights) = get_draft_itinerary_by_id(&draft_itinerary_id) else {
        return Err(Status::not_found("Itinerary ID not found or timed out."));
    };

    //
    // For each Draft flight in the itinerary, push to svc-storage
    //
    let mut confirmed_flight_plan_ids: Vec<String> = vec![];
    for fp in draft_itinerary_flights {
        if fp.fp_type == FlightPlanType::Existing {
            // TODO(R3) update svc-storage flight plan with new passenger count
            // let data = flight_plan::UpdateObject { ... }

            confirmed_flight_plan_ids.push(fp.fp_id);
            continue;
        }

        let confirmation = confirm_draft_flight_plan(fp.fp_id).await;

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
) -> Result<Response<CancelItineraryResponse>, Status> {
    let clients = get_clients().await;
    let itinerary_id = request.into_inner().id;
    grpc_info!("(cancel_itinerary) for id {}", &itinerary_id);

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
    clients
        .storage
        .itinerary
        .get_by_id(StorageId {
            id: itinerary_id.clone(),
        })
        .await?;

    //
    // TODO(R3) Don't allow cancellations within X minutes of the first flight
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
    clients.storage.itinerary.update(update_object).await?;

    grpc_info!(
        "cancel_itinerary with id {} cancelled in storage",
        &itinerary_id
    );

    let response = clients
        .storage
        .itinerary_flight_plan_link
        .get_linked_ids(StorageId {
            id: itinerary_id.clone(),
        })
        .await?;

    //
    // Cancel associated flight plans
    //
    //TODO: svc-storage currently doesn't check the FieldMask, so we'll
    //have to provide it with the right data object for now. Will now be handled
    //with temp code in for loop, but should be:
    //let mut flight_plan_data = flight_plan::Data::default();
    //flight_plan_data.flight_status = flight_plan::FlightStatus::Cancelled as i32;
    for id in response.into_inner().ids {
        // begin temp code
        let flight_plan = clients
            .storage
            .flight_plan
            .get_by_id(StorageId { id: id.clone() })
            .await?;
        let mut flight_plan_data = flight_plan.into_inner().data.unwrap();
        flight_plan_data.flight_status = flight_plan::FlightStatus::Cancelled as i32;
        // end temp code

        //
        // TODO(R4) Don't cancel flight plan if it exists in another itinerary
        //
        let request = flight_plan::UpdateObject {
            id: id.clone(),
            data: Some(flight_plan_data.clone()),
            mask: Some(FieldMask {
                paths: vec!["flight_status".to_string()],
            }),
        };
        let result = clients.storage.flight_plan.update(request).await;

        // Keep going even if there's a warning
        if result.is_err() {
            grpc_warn!("WARNING: Could not cancel flight plan with ID: {}", id);
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
    use crate::test_util::{ensure_storage_mock_data, get_vertiports_from_storage};
    use crate::{init_logger, Config};

    use super::*;
    use chrono::{TimeZone, Utc};

    #[tokio::test]
    async fn test_query_flight_plans_for_latest_arrival() {
        init_logger(&Config::try_from_env().unwrap_or_default());
        unit_test_info!("(test_query_flight_plans_for_latest_arrival) start");
        ensure_storage_mock_data().await;
        init_router().await;

        let latest_arrival_time: Timestamp = Utc
            .datetime_from_str("2022-10-26 14:30:00", "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .into();
        // our mock setup inserts only 3 flight_plans with an arrival date before "2022-10-26 14:30:00"
        let expected_number_returned = 3;

        let res = query_flight_plans_for_latest_arrival(latest_arrival_time).await;
        unit_test_debug!(
            "(test_query_flight_plans_for_latest_arrival) flight_plans returned: {:#?}",
            res
        );

        assert!(res.is_ok());
        assert_eq!(res.unwrap().len(), expected_number_returned);
        unit_test_info!("(test_query_flight_plans_for_latest_arrival) success");
    }

    #[tokio::test]
    async fn test_query_flight() {
        init_logger(&Config::try_from_env().unwrap_or_default());
        unit_test_info!("(test_query_flight) start");
        ensure_storage_mock_data().await;
        init_router().await;

        let vertiports = get_vertiports_from_storage().await;
        let res = query_flight(Request::new(QueryFlightRequest {
            is_cargo: false,
            persons: None,
            weight_grams: None,
            earliest_departure_time: Some(
                Utc.datetime_from_str("2022-10-25 11:20:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            latest_arrival_time: Some(
                Utc.datetime_from_str("2022-10-25 12:15:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            vertiport_depart_id: vertiports[0].id.clone(),
            vertiport_arrive_id: vertiports[1].id.clone(),
        }))
        .await;
        unit_test_debug!("(test_query_flight) query_flight result: {:?}", res);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().into_inner().itineraries.len(), 5);
        unit_test_info!("(test_query_flight) success");
    }

    #[tokio::test]
    async fn test_confirm_and_cancel_itinerary() {
        init_logger(&Config::try_from_env().unwrap_or_default());
        unit_test_info!("(test_confirm_and_cancel_itinerary) start");
        ensure_storage_mock_data().await;
        init_router().await;
        let res = confirm_itinerary(Request::new(ConfirmItineraryRequest {
            id: "itinerary1".to_string(),
            user_id: "".to_string(),
        }))
        .await;
        //test confirming a flight that does not exist will return an error
        assert_eq!(res.is_err(), true);

        let vertiports = get_vertiports_from_storage().await;
        let res = query_flight(Request::new(QueryFlightRequest {
            is_cargo: false,
            persons: None,
            weight_grams: None,
            earliest_departure_time: Some(
                Utc.datetime_from_str("2022-10-25 11:20:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            latest_arrival_time: Some(
                Utc.datetime_from_str("2022-10-25 12:15:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            vertiport_depart_id: vertiports[0].id.clone(),
            vertiport_arrive_id: vertiports[1].id.clone(),
        }))
        .await;
        unit_test_debug!(
            "(test_confirm_and_cancel_itinerary) query_flight result: {:#?}",
            res
        );
        assert!(res.is_ok());
        let id = res.unwrap().into_inner().itineraries[0].id.clone();
        let res = confirm_itinerary(Request::new(ConfirmItineraryRequest {
            id,
            user_id: "".to_string(),
        }))
        .await;
        assert!(res.is_ok());
        let confirm_response: ConfirmItineraryResponse = res.unwrap().into_inner();
        //test confirming a flight that does exist will return a success
        assert_eq!(confirm_response.confirmed, true);

        let id = confirm_response.id.clone();
        let res = cancel_itinerary(Request::new(Id { id })).await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap().into_inner().cancelled, true);

        unit_test_info!("(test_confirm_and_cancel_itinerary) success");
    }

    ///4. destination vertiport is available for about 15 minutes, no other restrictions
    /// - returns 2 flights (assuming 10 minutes needed for unloading, this can fit 2 flights
    /// if first is exactly at the beginning of 15 minute gap and second is exactly after 5 minutes)
    #[tokio::test]
    async fn test_query_flight_4_dest_vertiport_tight_availability_should_return_two_flights() {
        init_logger(&Config::try_from_env().unwrap_or_default());
        unit_test_info!("(test_query_flight_4_dest_vertiport_tight_availability_should_return_two_flights) start");
        ensure_storage_mock_data().await;
        init_router().await;

        let vertiports = get_vertiports_from_storage().await;
        let res = query_flight(Request::new(QueryFlightRequest {
            is_cargo: false,
            persons: None,
            weight_grams: None,
            earliest_departure_time: Some(
                Utc.datetime_from_str("2022-10-25 14:20:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            latest_arrival_time: Some(
                Utc.datetime_from_str("2022-10-25 15:10:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            vertiport_depart_id: vertiports[2].id.clone(),
            vertiport_arrive_id: vertiports[1].id.clone(),
        }))
        .await
        .unwrap();

        unit_test_debug!("(test_query_flight_4_dest_vertiport_tight_availability_should_return_two_flights) query_flight result: {:#?}", res);
        assert_eq!(res.into_inner().itineraries.len(), 2);
        unit_test_info!("(test_query_flight_4_dest_vertiport_tight_availability_should_return_two_flights) success");
    }

    ///5. source or destination vertiport doesn't have any vertipad free for the time range
    ///no flight plans returned
    #[tokio::test]
    async fn test_query_flight_5_dest_vertiport_no_availability_should_return_zero_flights() {
        init_logger(&Config::try_from_env().unwrap_or_default());
        unit_test_info!(
            "(test_query_flight_5_dest_vertiport_no_availability_should_return_zero_flights) start"
        );
        ensure_storage_mock_data().await;
        init_router().await;

        let vertiports = get_vertiports_from_storage().await;
        let res = query_flight(Request::new(QueryFlightRequest {
            is_cargo: false,
            persons: None,
            weight_grams: None,
            earliest_departure_time: Some(
                Utc.datetime_from_str("2022-10-26 14:00:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            latest_arrival_time: Some(
                Utc.datetime_from_str("2022-10-26 14:40:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            vertiport_depart_id: vertiports[1].id.clone(),
            vertiport_arrive_id: vertiports[0].id.clone(),
        }))
        .await;

        unit_test_debug!("(test_query_flight_5_dest_vertiport_no_availability_should_return_zero_flights) query_flight result: {:#?}", res);
        assert_eq!(
            res.unwrap_err()
                .message()
                .contains("No flight plans available"),
            true
        );
        unit_test_info!(
            "(test_query_flight_5_dest_vertiport_no_availability_should_return_zero_flights) success"
        );
    }

    ///6. vertiports are available but aircraft are not at the vertiport for the requested time
    /// but at least one aircraft is IN FLIGHT to requested vertiport for that time and has availability for a next flight.
    /// 	- skips all unavailable time slots (4) and returns only time slots from when aircraft is available (1)
    #[tokio::test]
    async fn test_query_flight_6_no_aircraft_at_vertiport() {
        init_logger(&Config::try_from_env().unwrap_or_default());
        unit_test_info!("(test_query_flight_6_no_aircraft_at_vertiport) start");
        ensure_storage_mock_data().await;
        init_router().await;

        let vertiports = get_vertiports_from_storage().await;
        let res = query_flight(Request::new(QueryFlightRequest {
            is_cargo: false,
            persons: None,
            weight_grams: None,
            earliest_departure_time: Some(
                Utc.datetime_from_str("2022-10-26 14:15:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            latest_arrival_time: Some(
                Utc.datetime_from_str("2022-10-26 15:00:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            vertiport_depart_id: vertiports[0].id.clone(),
            vertiport_arrive_id: vertiports[2].id.clone(),
        }))
        .await
        .unwrap()
        .into_inner();

        unit_test_debug!(
            "(test_query_flight_6_no_aircraft_at_vertiport) query_flight result: {:#?}",
            res
        );
        assert_eq!(res.itineraries.len(), 1);
        assert_eq!(res.itineraries[0].deadhead_flight_plans.len(), 0);
        unit_test_info!("(test_query_flight_6_no_aircraft_at_vertiport) success");
    }

    /// 7. vertiports are available but aircraft are not at the vertiport for the requested time
    /// but at least one aircraft is PARKED at other vertiport for the "requested time - N minutes"
    #[tokio::test]
    async fn test_query_flight_7_deadhead_flight_of_parked_vehicle() {
        init_logger(&Config::try_from_env().unwrap_or_default());
        unit_test_info!("(test_query_flight_7_deadhead_flight_of_parked_vehicle) start");
        ensure_storage_mock_data().await;
        init_router().await;

        let vertiports = get_vertiports_from_storage().await;
        let res = query_flight(Request::new(QueryFlightRequest {
            is_cargo: false,
            persons: None,
            weight_grams: None,
            earliest_departure_time: Some(
                Utc.datetime_from_str("2022-10-26 16:00:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            latest_arrival_time: Some(
                Utc.datetime_from_str("2022-10-26 16:30:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            vertiport_depart_id: vertiports[2].id.clone(),
            vertiport_arrive_id: vertiports[0].id.clone(),
        }))
        .await
        .unwrap()
        .into_inner();

        unit_test_debug!(
            "(test_query_flight_7_deadhead_flight_of_parked_vehicle) query_flight result: {:#?}",
            res
        );
        assert_eq!(res.itineraries.len(), 1);
        assert_eq!(res.itineraries[0].deadhead_flight_plans.len(), 1);
        unit_test_info!("(test_query_flight_7_deadhead_flight_of_parked_vehicle) success");
    }

    /// 8. vertiports are available but aircraft are not at the vertiport for the requested time
    /// but at least one aircraft is EN ROUTE to another vertiport for the "requested time - N minutes - M minutes"
    #[tokio::test]
    async fn test_query_flight_8_deadhead_flight_of_in_flight_vehicle() {
        init_logger(&Config::try_from_env().unwrap_or_default());
        unit_test_info!("(test_query_flight_8_deadhead_flight_of_in_flight_vehicle) start");
        ensure_storage_mock_data().await;
        init_router().await;

        let vertiports = get_vertiports_from_storage().await;
        let res = query_flight(Request::new(QueryFlightRequest {
            is_cargo: false,
            persons: None,
            weight_grams: None,
            earliest_departure_time: Some(
                Utc.datetime_from_str("2022-10-27 12:30:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            latest_arrival_time: Some(
                Utc.datetime_from_str("2022-10-27 13:30:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            vertiport_depart_id: vertiports[1].id.clone(),
            vertiport_arrive_id: vertiports[0].id.clone(),
        }))
        .await
        .unwrap()
        .into_inner();

        unit_test_debug!(
            "(test_query_flight_8_deadhead_flight_of_in_flight_vehicle) query_flight result: {:#?}",
            res
        );
        assert_eq!(res.itineraries.len(), 2);
        assert_eq!(res.itineraries[0].deadhead_flight_plans.len(), 1);
        unit_test_info!("(test_query_flight_8_deadhead_flight_of_in_flight_vehicle) success");
    }

    /* TODO: R4 refactor code and re-implement this test
    /// 9. destination vertiport is not available because of capacity
    /// - if at requested time all pads are occupied and at least one is parked (not loading/unloading),
    /// a extra flight plan should be created to move idle aircraft to the nearest unoccupied vertiport
    /// (or to preferred vertiport in hub and spoke model).
    #[tokio::test]
    async fn test_query_flight_9_deadhead_destination_flight_no_capacity_at_destination_vertiport()
    {
        init_logger(&Config::try_from_env().unwrap_or_default());
        unit_test_info!("(test_query_flight_9_deadhead_destination_flight_no_capacity_at_destination_vertiport) start");
        ensure_storage_mock_data().await;
        init_router().await;

        let vertiports = get_vertiports_from_storage().await;
        let res = query_flight(Request::new(QueryFlightRequest {
            is_cargo: false,
            persons: None,
            weight_grams: None,
            earliest_departure_time: Some(
                Utc.datetime_from_str("2022-10-27 15:10:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            latest_arrival_time: Some(
                Utc.datetime_from_str("2022-10-27 16:00:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .into(),
            ),
            vertiport_depart_id: vertiports[1].id.clone(),
            vertiport_arrive_id: vertiports[3].id.clone(),
        }))
        .await
        .unwrap()
        .into_inner();

        unit_test_debug!(
            "(test_query_flight_9_deadhead_destination_flight_no_capacity_at_destination_vertiport) query_flight result: {:#?}",
            res
        );
        assert_eq!(res.itineraries.len(), 1);
        assert_eq!(res.itineraries[0].deadhead_flight_plans.len(), 1);
        unit_test_info!("(test_query_flight_9_deadhead_destination_flight_no_capacity_at_destination_vertiport) success");
    }
    */
}
