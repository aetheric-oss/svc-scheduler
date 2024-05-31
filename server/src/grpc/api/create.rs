//! This module contains the gRPC confirm_itinerary endpoint implementation.

use crate::grpc::server::grpc_server::{
    CreateItineraryRequest, TaskAction, TaskMetadata, TaskResponse, TaskStatus,
};
use num_traits::FromPrimitive;
use tonic::Status;

use crate::router::flight_plan::{FlightPlanError, FlightPlanSchedule};
use crate::tasks::pool::RedisPool;
use crate::tasks::{Task, TaskBody};

use lib_common::uuid::Uuid;

/// Creates an itinerary from a list of flight plans.
/// The flight plans provided are expected to be the valid output from the `query_flight` endpoint.
/// Invalid flight plans will be quickly rejected.
pub async fn create_itinerary(request: CreateItineraryRequest) -> Result<TaskResponse, Status> {
    let priority = FromPrimitive::from_i32(request.priority).ok_or_else(|| {
        let error_msg = "Invalid priority provided";
        grpc_error!("{error_msg}: {}", request.priority);
        Status::invalid_argument(format!("{error_msg}."))
    })?;

    let user_id = Uuid::parse_str(&request.user_id.clone()).map_err(|e| {
        let error_msg = "Invalid user ID provided";
        grpc_error!("{error_msg}: {e}");
        Status::invalid_argument(format!("{error_msg}."))
    })?;

    let schedules = request
        .flight_plans
        .into_iter()
        .map(FlightPlanSchedule::try_from)
        .collect::<Result<Vec<FlightPlanSchedule>, FlightPlanError>>()
        .map_err(|e| {
            let error_msg = "Invalid flight plans provided";
            grpc_error!("{error_msg}: {e}");
            Status::invalid_argument(format!("{error_msg}."))
        })?;

    // Set to expire if it hasn't been acted on by the start of the first flight plan
    let expiry = schedules
        .iter()
        .min()
        .ok_or(Status::invalid_argument("No flight plans provided."))?
        .origin_timeslot_start;

    grpc_debug!("Default expiry: {expiry}.");

    let expiry = match request.expiry {
        // if an earlier expiry was provided in the request, use that instead
        Some(request_expiry) => {
            grpc_debug!("Request expiry: {expiry}.");
            expiry.min(request_expiry.into())
        }
        None => expiry,
    };

    grpc_debug!("Task expiry set to: {expiry}.");

    let task = Task {
        metadata: TaskMetadata {
            status: TaskStatus::Queued as i32,
            status_rationale: None,
            action: TaskAction::CreateItinerary as i32,
            user_id: user_id.to_string(),
            result: None,
        },
        body: TaskBody::CreateItinerary(schedules),
    };

    // Add the task to the scheduler:tasks table
    let mut pool = crate::tasks::pool::get_pool().await.ok_or_else(|| {
        grpc_error!("Couldn't get the redis pool.");
        Status::internal("Internal error.")
    })?;

    let task_id = pool.new_task(&task, priority, expiry).await.map_err(|e| {
        let error_msg = "Could not create new task.";
        grpc_error!("{error_msg}: {e}");
        Status::internal(format!("{error_msg}."))
    })?;

    grpc_info!("Created new task with ID: {}", task_id);
    Ok(TaskResponse {
        task_id,
        task_metadata: Some(task.metadata),
    })
}
