//! This module contains the gRPC confirm_itinerary endpoint implementation.

use svc_storage_client_grpc::prelude::*;

use crate::grpc::client::get_clients;
use crate::grpc::server::grpc_server::{ConfirmItineraryRequest, ConfirmItineraryResponse};
// TODO(R4): Compliance service will handle this without needing a request from scheduler
use chrono::Utc;
use svc_compliance_client_grpc::client::FlightPlanRequest;
use svc_compliance_client_grpc::service::Client as ComplianceServiceClient;
use svc_storage_client_grpc::prelude::{flight_plan, IdList};
use tonic::{Request, Response, Status};

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
    let _ = super::remove_draft_itinerary_by_id(&draft_itinerary_id);
    Ok(db_itinerary)
}

/// Confirms a flight plan by registering it with svc-storage
/// After confirmation, the flight plan will be removed from local cache
async fn confirm_draft_flight_plan(flight_plan_id: String) -> Result<flight_plan::Object, Status> {
    let clients = get_clients().await;

    let Some(flight_plan) = super::get_draft_fp_by_id(&flight_plan_id) else {
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
    let _ = super::remove_draft_fp_by_id(&flight_plan_id);

    // TODO(R4): Compliance service will handle this without needing a request from scheduler
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

    let Some(draft_itinerary_flights) = super::get_draft_itinerary_by_id(&draft_itinerary_id) else {
        return Err(Status::not_found("Itinerary ID not found or timed out."));
    };

    //
    // For each Draft flight in the itinerary, push to svc-storage
    //
    let mut confirmed_flight_plan_ids: Vec<String> = vec![];
    for fp_id in draft_itinerary_flights {
        // TODO(R4) - Check if flight plan is already in database
        // if fp.fp_type == FlightPlanType::Existing {
        //     // TODO(R4) - update record with new parcel/passenger data

        //     confirmed_flight_plan_ids.push(fp.fp_id);
        //     continue;
        // }

        // TODO(R4) - insert all flight plans at same time (one transaction)
        //  so if one fails they all fail
        let confirmation = confirm_draft_flight_plan(fp_id).await;
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
                confirmation_time: Some(Utc::now().into()),
            };

            Ok(Response::new(response))
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::grpc::api::cancel_itinerary::cancel_itinerary;
    use crate::grpc::api::query_flight::query_flight;
    use crate::grpc::server::grpc_server::QueryFlightRequest;
    use crate::init_logger;
    use crate::test_util::{ensure_storage_mock_data, get_vertiports_from_storage};
    use chrono::{Duration, Utc};

    #[cfg(feature = "stub_backends")]
    #[tokio::test]
    async fn test_confirm_and_cancel_itinerary() {
        init_logger(&Config::try_from_env().unwrap_or_default());
        unit_test_info!("(test_confirm_and_cancel_itinerary) start");
        ensure_storage_mock_data().await;
        let res = confirm_itinerary(Request::new(ConfirmItineraryRequest {
            id: "itinerary1".to_string(),
            user_id: "".to_string(),
        }))
        .await;
        //test confirming a flight that does not exist will return an error
        assert_eq!(res.is_err(), true);

        let date = (Utc::now() + Duration::days(10)).date_naive();
        let dt_start = date.and_hms_opt(0, 0, 0).unwrap();

        let vertiports = get_vertiports_from_storage().await;
        println!("vertiports: {:#?}", vertiports);
        let res = query_flight(Request::new(QueryFlightRequest {
            is_cargo: true,
            persons: None,
            weight_grams: Some(100),
            earliest_departure_time: Some(dt_start.into()),
            latest_arrival_time: Some((dt_start + chrono::Duration::hours(1)).into()),
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
        let res = cancel_itinerary(Request::new(crate::grpc::server::grpc_server::Id { id })).await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap().into_inner().cancelled, true);

        unit_test_info!("(test_confirm_and_cancel_itinerary) success");
    }
}
