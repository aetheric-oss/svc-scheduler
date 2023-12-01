//! This module contains the gRPC confirm_itinerary endpoint implementation.

use crate::grpc::server::grpc_server::{
    CreateItineraryRequest, TaskAction, TaskMetadata, TaskResponse, TaskStatus,
};
use chrono::{DateTime, Utc};
use num_traits::FromPrimitive;
use std::cmp::min;
use tonic::Status;

use crate::router::flight_plan::{FlightPlanError, FlightPlanSchedule};
use crate::tasks::pool::RedisPool;
use crate::tasks::{Task, TaskBody};

/// Creates an itinerary from a list of flight plans.
/// The flight plans provided are expected to be the valid output from the `query_flight` endpoint.
/// Invalid flight plans will be quickly rejected.
pub async fn create_itinerary(request: CreateItineraryRequest) -> Result<TaskResponse, Status> {
    let priority = match FromPrimitive::from_i32(request.priority) {
        Some(p) => p,
        None => {
            let error_msg = "Invalid priority provided";
            grpc_error!("(create_itinerary) {error_msg}: {}", request.priority);
            return Err(Status::invalid_argument(format!("{error_msg}.")));
        }
    };

    let Ok(schedules): Result<Vec<FlightPlanSchedule>, FlightPlanError> = request
        .flight_plans
        .into_iter()
        .map(FlightPlanSchedule::try_from)
        .collect()
    else {
        return Err(Status::invalid_argument("Invalid flight plans provided."));
    };

    // Set to expire if it hasn't been acted on by the start of the first flight plan
    let expiry = match schedules.iter().min() {
        Some(fp) => fp.origin_timeslot_start,
        None => {
            return Err(Status::invalid_argument("No flight plans provided."));
        }
    };

    let expiry = match request.expiry {
        // if an earlier expiry was provided in the request, use that instead
        Some(request_expiry) => {
            let request_expiry: DateTime<Utc> = request_expiry.into();
            min(request_expiry, expiry)
        }
        None => expiry,
    };

    let task = Task {
        metadata: TaskMetadata {
            status: TaskStatus::Queued as i32,
            status_rationale: None,
            action: TaskAction::CreateItinerary as i32,
        },
        body: TaskBody::CreateItinerary(schedules),
    };

    // Add the task to the scheduler:tasks table
    let Some(mut pool) = crate::tasks::pool::get_pool().await else {
        grpc_error!("(create_itinerary) Couldn't get the redis pool.");
        return Err(Status::internal("Internal error."));
    };

    match pool.new_task(&task, priority, expiry).await {
        Ok(task_id) => {
            grpc_info!("(create_itinerary) Created new task with ID: {}", task_id);

            Ok(TaskResponse {
                task_id,
                task_metadata: Some(task.metadata),
            })
        }
        Err(e) => {
            let error_msg = "Could not create new task.";
            grpc_error!("(create_itinerary) {error_msg}: {e}");
            Err(Status::internal(format!("{error_msg}.")))
        }
    }
}
