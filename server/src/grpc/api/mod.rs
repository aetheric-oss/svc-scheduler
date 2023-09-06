//! gRPC API parent module

pub mod cancel_itinerary;
pub mod confirm_itinerary;
pub mod query_flight;

use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Mutex;
use svc_storage_client_grpc::prelude::flight_plan;

/// Time to wait for a flight plan to be confirmed before cancelling it
const ITINERARY_EXPIRATION_S: u64 = 30;

lazy_static! {
    static ref UNCONFIRMED_ITINERARIES: Mutex<HashMap<String, Vec<String>>> =
        Mutex::new(HashMap::new());
    static ref UNCONFIRMED_FLIGHT_PLANS: Mutex<HashMap<String, flight_plan::Data>> =
        Mutex::new(HashMap::new());
}

/// gets a hashmap of unconfirmed itineraries
/// "itineraries" are a list of flight plan IDs, which can represent DRAFT
///  (in memory) or EXISTING (in database) flight plans
pub fn unconfirmed_itineraries() -> &'static Mutex<HashMap<String, Vec<String>>> {
    &UNCONFIRMED_ITINERARIES
}

/// gets a hashmap of unconfirmed flight plans
pub fn unconfirmed_flight_plans() -> &'static Mutex<HashMap<String, flight_plan::Data>> {
    &UNCONFIRMED_FLIGHT_PLANS
}

/// Gets itinerary from hash map of unconfirmed itineraries
pub fn get_draft_itinerary_by_id(id: &str) -> Option<Vec<String>> {
    unconfirmed_itineraries()
        .lock()
        .expect("Mutex Lock Error getting itinerary from temp storage")
        .get(id)
        .cloned()
}

/// Gets flight plan from hash map of unconfirmed flight plans
pub fn get_draft_fp_by_id(id: &str) -> Option<flight_plan::Data> {
    unconfirmed_flight_plans()
        .lock()
        .expect("Mutex Lock Error getting flight plan from temp storage")
        .get(id)
        .cloned()
}

/// spawns a thread that will cancel the itinerary after a certain amount of time (ITINERARY_EXPIRATION_S)
fn cancel_itinerary_after_timeout(id: String) {
    tokio::spawn(async move {
        tokio::time::sleep(core::time::Duration::from_secs(ITINERARY_EXPIRATION_S)).await;
        remove_draft_itinerary_by_id(&id);
        grpc_debug!("(cancel_itinerary_after_timeout) Flight plan {} was not confirmed in time, cancelling.", id);
    });
}

/// Removes flight plan from hash map of unconfirmed flight plans
fn remove_draft_fp_by_id(id: &str) -> bool {
    let mut flight_plans = unconfirmed_flight_plans()
        .lock()
        .expect("(remove_draft_fp_by_id) mutex Lock Error removing flight plan from temp storage.");

    match flight_plans.remove(id) {
        Some(_) => {
            grpc_debug!(
                "(remove_draft_fp_by_id) with id {} removed from local cache.",
                &id
            );
            true
        }
        _ => {
            grpc_debug!(
                "(remove_draft_fp_by_id) no such flight plan with ID {} in cache.",
                &id
            );
            false
        }
    }
}

/// Removes itinerary from hash map of unconfirmed flight plans
fn remove_draft_itinerary_by_id(id: &str) -> bool {
    let mut itineraries = unconfirmed_itineraries().lock().expect(
        "(remove_draft_itinerary_by_id) mutex Lock Error removing itinerary from temp storage.",
    );

    let Some(itinerary) = itineraries.get(id) else {
        grpc_debug!("(remove_draft_itinerary_by_id) no such itinerary with ID {} in cache.", &id);
        return false;
    };

    // Remove draft flight plans associated with this itinerary
    for fp_id in itinerary {
        // TODO(R4) - Remove flight plans if they are draft and only in
        //  one itinerary
        // if fp.fp_type == FlightPlanType::Draft {
        // Ignore if not found
        let _ = remove_draft_fp_by_id(fp_id);
        // }
    }

    itineraries.remove(id);

    grpc_info!(
        "(remove_draft_itinerary_by_id) cancel_itinerary with id {} removed from local cache.",
        &id
    );
    true
}
