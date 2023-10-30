//! This module contains the gRPC cancel_itinerary endpoint implementation.

use crate::grpc::server::grpc_server::{
    CancelItineraryRequest, TaskAction, TaskMetadata, TaskResponse, TaskStatus,
};
use crate::tasks::pool::RedisPool;
use crate::tasks::{Task, TaskBody};
use chrono::{Duration, Utc};
use num_traits::FromPrimitive;
use tonic::Status;
use uuid::Uuid;

/// Cancels an itinerary
pub async fn cancel_itinerary(request: CancelItineraryRequest) -> Result<TaskResponse, Status> {
    let itinerary_id = match Uuid::parse_str(&request.itinerary_id) {
        Ok(id) => id,
        Err(_) => {
            return Err(Status::invalid_argument("Invalid itinerary ID."));
        }
    };

    let Some(priority) = FromPrimitive::from_i32(request.priority) else {
        return Err(Status::invalid_argument("Invalid priority provided."));
    };

    // TODO(R4): Get the itinerary start time from storage
    // For now hardcode next hour
    let expiry = Utc::now() + Duration::hours(1);
    let task = Task {
        metadata: TaskMetadata {
            status: TaskStatus::Queued as i32,
            status_rationale: None,
            action: TaskAction::CancelItinerary as i32,
        },
        body: TaskBody::CancelItinerary(itinerary_id),
    };

    let Some(mut pool) = crate::tasks::pool::get_pool().await else {
        grpc_error!("(cancel_itinerary) Couldn't get the redis pool.");
        return Err(Status::internal("Internal error."));
    };

    // Add the task to the scheduler:tasks table
    match pool.new_task(&task, priority, expiry).await {
        Ok(task_id) => {
            grpc_info!("(cancel_itinerary) Created new task with ID: {}", task_id);

            Ok(TaskResponse {
                task_id,
                task_metadata: Some(task.metadata),
            })
        }
        Err(e) => {
            let error_msg = "Could not create new task.";
            grpc_error!("(cancel_itinerary) {error_msg}: {e}");
            Err(Status::internal(format!("{error_msg}.")))
        }
    }
}
