//! This module contains the gRPC cancel_itinerary endpoint implementation.

use crate::grpc::client::get_clients;
use crate::grpc::server::grpc_server::{CancelItineraryResponse, Id};
use chrono::Utc;
use svc_storage_client_grpc::prelude::Id as StorageId;
use svc_storage_client_grpc::prelude::*;
use tonic::{Request, Response, Status};
use uuid::Uuid;

/// Cancels a draft or confirmed flight plan
pub async fn cancel_itinerary(
    request: Request<Id>,
) -> Result<Response<CancelItineraryResponse>, Status> {
    let itinerary_id = match Uuid::parse_str(&request.into_inner().id) {
        Ok(id) => id.to_string(),
        Err(_) => {
            return Err(Status::invalid_argument("Invalid itinerary ID."));
        }
    };

    let clients = get_clients().await;
    grpc_info!("(cancel_itinerary) for id {}.", &itinerary_id);

    //
    // Look within unconfirmed itineraries
    //
    if super::remove_draft_itinerary_by_id(&itinerary_id) {
        let response = CancelItineraryResponse {
            id: itinerary_id,
            cancelled: true,
            cancellation_time: Some(Utc::now().into()),
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
    // TODO(R4) Don't allow cancellations within X minutes of the first flight
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
        "(cancel_itinerary) cancel_itinerary with id {} cancelled in storage.",
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
    //TODO(R4): svc-storage currently doesn't check the FieldMask, so we'll
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

        let Some(mut flight_plan_data) = flight_plan.into_inner().data else {
            grpc_warn!(
                "(cancel_itinerary) WARNING: Could not cancel flight plan with ID: {}",
                id
            );
            continue;
        };

        flight_plan_data.flight_status = flight_plan::FlightStatus::Cancelled as i32;
        // end temp code

        //
        // TODO(R4): Don't cancel flight plan if it exists in another itinerary
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
            grpc_warn!(
                "(cancel_itinerary) WARNING: Could not cancel flight plan with ID: {}",
                id
            );
        }
    }

    //
    // Reply
    //
    let response = CancelItineraryResponse {
        id: itinerary_id,
        cancelled: true,
        cancellation_time: Some(Utc::now().into()),
        reason: "user cancelled".into(),
    };
    Ok(Response::new(response))
}
