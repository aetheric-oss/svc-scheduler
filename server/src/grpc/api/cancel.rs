//! This module contains the gRPC cancel_itinerary endpoint implementation.

use crate::grpc::server::grpc_server::{
    CancelItineraryRequest, TaskAction, TaskMetadata, TaskResponse, TaskStatus,
};
use crate::tasks::pool::RedisPool;
use crate::tasks::{Task, TaskBody};
use lib_common::time::{Duration, Utc};
use lib_common::uuid::to_uuid;
use num_traits::FromPrimitive;
use std::fmt::{self, Display, Formatter};

/// The expiry of the cancellation task is set to the current time plus one hour.
/// Cancellations should be handled first, so this should be enough time
const CANCELLATION_EXPIRY_MINUTES: i64 = 60;

/// Errors that can occur when cancelling an itinerary
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum CancelItineraryError {
    /// Invalid itinerary ID provided
    ItineraryId,

    /// Invalid user ID provided
    UserId,

    /// Invalid priority provided
    Priority(i32),

    /// Error creating time delta
    TimeDelta,

    /// Internal error
    InternalError,

    /// Error getting the redis pool
    RedisPool,

    /// Error creating a new task
    TaskCreation,
}

impl Display for CancelItineraryError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Error cancelling itinerary: ")?;
        match self {
            Self::ItineraryId => write!(f, "Invalid itinerary ID provided."),
            Self::UserId => write!(f, "Invalid user ID provided."),
            Self::Priority(p) => write!(f, "Invalid priority provided: {p}."),
            Self::TimeDelta => write!(f, "Error creating time delta."),
            Self::InternalError => write!(f, "Internal error."),
            Self::RedisPool => write!(f, "Couldn't get the redis pool."),
            Self::TaskCreation => write!(f, "Could not create new task."),
        }
    }
}

/// Cancels an itinerary
pub async fn cancel_itinerary(
    request: CancelItineraryRequest,
) -> Result<TaskResponse, CancelItineraryError> {
    let itinerary_id = to_uuid(&request.itinerary_id).ok_or(CancelItineraryError::ItineraryId)?;

    let user_id = to_uuid(&request.user_id).ok_or(CancelItineraryError::UserId)?;

    let priority = FromPrimitive::from_i32(request.priority)
        .ok_or(CancelItineraryError::Priority(request.priority))?;

    // TODO(R5): Get the itinerary start time from storage
    // For now hardcode next hour
    #[cfg(not(tarpaulin_include))]
    // no_coverage: (R5) this can't fail. See [`tests::test_cancellation_expiry_minutes`] for coverage.
    let delta = Duration::try_minutes(CANCELLATION_EXPIRY_MINUTES).ok_or_else(|| {
        grpc_error!("error creating time delta.");
        CancelItineraryError::TimeDelta
    })?;

    let expiry = Utc::now() + delta;
    let task = Task {
        metadata: TaskMetadata {
            status: TaskStatus::Queued as i32,
            status_rationale: None,
            action: TaskAction::CancelItinerary as i32,
            user_id: user_id.to_string(),
            result: None,
        },
        body: TaskBody::CancelItinerary(itinerary_id),
    };

    #[cfg(not(tarpaulin_include))]
    // no_coverage: (R5) need redis backend to test this
    {
        let mut pool = crate::tasks::pool::get_pool().await.ok_or_else(|| {
            grpc_error!("Couldn't get the redis pool.");
            CancelItineraryError::RedisPool
        })?;

        // Add the task to the scheduler:tasks table
        let task_id = pool.new_task(&task, priority, expiry).await.map_err(|e| {
            grpc_error!("Could not create new task: {e}");
            CancelItineraryError::TaskCreation
        })?;

        grpc_info!("Created new task with ID: {}", task_id);
        Ok(TaskResponse {
            task_id,
            task_metadata: Some(task.metadata),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use tonic::Status;

    #[test]
    fn test_cancellation_expiry_minutes() {
        Duration::try_minutes(CANCELLATION_EXPIRY_MINUTES).unwrap();
    }

    #[test]
    fn test_cancel_itinerary_error_display() {
        assert_eq!(
            format!("{}", CancelItineraryError::ItineraryId),
            "Error cancelling itinerary: Invalid itinerary ID provided."
        );
        assert_eq!(
            format!("{}", CancelItineraryError::UserId),
            "Error cancelling itinerary: Invalid user ID provided."
        );
        assert_eq!(
            format!("{}", CancelItineraryError::Priority(1)),
            "Error cancelling itinerary: Invalid priority provided: 1."
        );
        assert_eq!(
            format!("{}", CancelItineraryError::TimeDelta),
            "Error cancelling itinerary: Error creating time delta."
        );
        assert_eq!(
            format!("{}", CancelItineraryError::InternalError),
            "Error cancelling itinerary: Internal error."
        );
        assert_eq!(
            format!("{}", CancelItineraryError::RedisPool),
            "Error cancelling itinerary: Couldn't get the redis pool."
        );
        assert_eq!(
            format!("{}", CancelItineraryError::TaskCreation),
            "Error cancelling itinerary: Could not create new task."
        );
    }
}
