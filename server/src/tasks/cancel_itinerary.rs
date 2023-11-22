//! This module contains the gRPC cancel_itinerary endpoint implementation.

use crate::grpc::client::get_clients;
use crate::grpc::server::grpc_server::TaskStatus;
use crate::tasks::{Task, TaskAction, TaskBody, TaskError};
use num_traits::FromPrimitive;
use svc_storage_client_grpc::prelude::Id as StorageId;
use svc_storage_client_grpc::prelude::*;

/// Cancels an itinerary
pub async fn cancel_itinerary(task: &mut Task) -> Result<(), TaskError> {
    let Some(TaskAction::CancelItinerary) = FromPrimitive::from_i32(task.metadata.action) else {
        tasks_error!(
            "(cancel_itinerary) Invalid task action: {}",
            task.metadata.action
        );
        return Err(TaskError::InvalidMetadata);
    };

    let TaskBody::CancelItinerary(itinerary_id) = &task.body else {
        tasks_error!("(cancel_itinerary) Invalid task body: {:?}", task.body);
        return Err(TaskError::InvalidData);
    };

    tasks_info!("(cancel_itinerary) for id {}.", &itinerary_id);

    let clients = get_clients().await;
    let itinerary = match clients
        .storage
        .itinerary
        .get_by_id(Id {
            id: itinerary_id.to_string(),
        })
        .await
    {
        Ok(i) => i.into_inner(),
        Err(e) => {
            tasks_warn!("(cancel_itinerary) Could not find itinerary with ID {itinerary_id}: {e}",);
            return Err(TaskError::InvalidData);
        }
    };

    let Some(data) = itinerary.data else {
        tasks_warn!(
            "(cancel_itinerary) Itinerary has invalid data: {}",
            itinerary_id
        );
        return Err(TaskError::Internal);
    };

    if data.status != itinerary::ItineraryStatus::Active as i32 {
        tasks_warn!(
            "(cancel_itinerary) Itinerary with ID: {} is not active.",
            itinerary_id
        );

        return Err(TaskError::AlreadyProcessed);
    }

    //
    // TODO(R4) Don't allow cancellations within X minutes of the first flight
    //

    //
    // TODO(R4): Heal the gap created by the removed flight plans
    //

    //
    // Remove itinerary
    //
    let update_object = itinerary::UpdateObject {
        id: itinerary_id.to_string(),
        data: Some(itinerary::Data {
            status: itinerary::ItineraryStatus::Cancelled as i32,
            ..data.clone()
        }),
        mask: Some(FieldMask {
            paths: vec!["status".to_string()],
        }),
    };

    if let Err(e) = clients.storage.itinerary.update(update_object).await {
        tasks_warn!("(cancel_itinerary) Could not cancel itinerary with ID {itinerary_id}: {e}",);

        return Err(TaskError::Internal);
    };

    tasks_info!(
        "(cancel_itinerary) cancel_itinerary with id {} cancelled in storage.",
        &itinerary_id
    );

    let response = match clients
        .storage
        .itinerary_flight_plan_link
        .get_linked_ids(StorageId {
            id: itinerary_id.to_string(),
        })
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tasks_warn!(
                "(cancel_itinerary) Could not get flight plans for itinerary with ID {itinerary_id}: {e}",
            );

            return Err(TaskError::Internal);
        }
    };

    //
    // Cancel associated flight plans
    //
    // TODO(R4): svc-storage currently doesn't check the FieldMask, so we'll
    // have to provide it with the right data object for now. Will now be handled
    // with temp code in for loop, but should be:
    // let mut flight_plan_data = flight_plan::Data::default();
    // flight_plan_data.flight_status = flight_plan::FlightStatus::Cancelled as i32;
    for id in response.into_inner().ids {
        // begin temp code
        let Ok(flight_plan) = clients
            .storage
            .flight_plan
            .get_by_id(StorageId { id: id.clone() })
            .await
        else {
            tasks_warn!(
                "(cancel_itinerary) WARNING: Could not get flight plan with ID: {}",
                id
            );

            continue;
        };

        let Some(mut flight_plan_data) = flight_plan.into_inner().data else {
            tasks_warn!(
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

        match clients.storage.flight_plan.update(request).await {
            Ok(_) => {
                tasks_info!("(cancel_itinerary) Cancelled flight plan with ID: {id}");
            }
            Err(e) => {
                tasks_error!(
                    "(cancel_itinerary) WARNING: Could not cancel flight plan with ID: {id}; {e}"
                );
            }
        }
    }

    task.metadata.status = TaskStatus::Complete.into();

    // TODO(R4): Internal cancellations should change this to InternalCancelled
    // task.body.status_rationale = TaskStatusRationale::ClientCancelled;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{TaskAction, TaskBody, TaskMetadata};
    use uuid::Uuid;

    type TaskResult = Result<(), TaskError>;

    #[tokio::test]
    async fn ut_cancel_itinerary_invalid_task_body() -> TaskResult {
        let mut task = Task {
            metadata: TaskMetadata {
                action: TaskAction::CancelItinerary as i32,
                ..Default::default()
            },
            body: TaskBody::CreateItinerary(vec![]),
        };

        let e = cancel_itinerary(&mut task).await.unwrap_err();
        assert_eq!(e, TaskError::InvalidData);

        Ok(())
    }

    #[tokio::test]
    async fn ut_cancel_itinerary_invalid_metadata() -> TaskResult {
        let mut task = Task {
            metadata: TaskMetadata {
                action: TaskAction::CreateItinerary as i32,
                ..Default::default()
            },
            body: TaskBody::CancelItinerary(Uuid::new_v4()),
        };

        let e = cancel_itinerary(&mut task).await.unwrap_err();
        assert_eq!(e, TaskError::InvalidMetadata);

        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "stub_client")]
    async fn ut_cancel_itinerary_invalid_itinerary_id() -> TaskResult {
        let mut task = Task {
            metadata: TaskMetadata {
                action: TaskAction::CancelItinerary as i32,
                ..Default::default()
            },
            body: TaskBody::CancelItinerary(uuid::Uuid::new_v4()),
        };

        let e = cancel_itinerary(&mut task).await.unwrap_err();
        assert_eq!(e, TaskError::InvalidData);

        Ok(())
    }
}
