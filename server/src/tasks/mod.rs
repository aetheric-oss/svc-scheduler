//! gRPC
//! provides Redis implementations for caching layer

#[macro_use]
pub mod macros;
pub mod pool;

mod cancel_itinerary;
mod create_itinerary;

use cancel_itinerary::cancel_itinerary;
use create_itinerary::create_itinerary;

use crate::grpc::server::grpc_server::{TaskAction, TaskMetadata, TaskStatus, TaskStatusRationale};
use crate::router::flight_plan::FlightPlanSchedule;
use crate::tasks::pool::RedisPool;
use deadpool_redis::redis::{self, FromRedisValue, ToRedisArgs};
use lib_common::time::{Duration, Utc};
use lib_common::uuid::Uuid;
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Result as FmtResult};

/// How long to keep a task in memory after it's been processed
const TASK_KEEPALIVE_DURATION_MINUTES: i64 = 60;
/// How long to sleep (in milliseconds) if the queue is empty
const IDLE_DURATION_MS: u64 = 1000;

/// The required information to complete a task
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TaskBody {
    /// Cancel an itinerary
    CancelItinerary(Uuid),

    /// Create an itinerary
    CreateItinerary(Vec<FlightPlanSchedule>),
}

/// Complete information about a task
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Task {
    /// Metadata about the task
    pub metadata: TaskMetadata,

    /// Details about the task
    pub body: TaskBody,
}

impl FromRedisValue for Task {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        let redis::Value::Data(data) = v else {
            return Err(redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "Unexpected Redis value",
            )));
        };

        let Ok(task): Result<Task, serde_json::Error> = serde_json::from_slice(data) else {
            return Err(redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "Invalid JSON",
            )));
        };

        Ok(task)
    }
}

impl ToRedisArgs for Task {
    fn write_redis_args<W: ?Sized>(&self, out: &mut W)
    where
        W: redis::RedisWrite,
    {
        let Ok(result) = serde_json::to_string(&self) else {
            tasks_warn!("error serializing task.");
            return;
        };

        out.write_arg(result.as_bytes());
    }
}

/// Errors that can occur when processing a task
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TaskError {
    /// Task id was not found
    NotFound,

    /// Internal error with updating task
    Internal,

    /// Task was already processed
    AlreadyProcessed,

    /// Invalid metadata provided,
    Metadata,

    /// Invalid User ID provided
    UserId,

    /// Invalid data provided
    Data,

    /// Schedule Conflict
    ScheduleConflict,
}

impl Display for TaskError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            TaskError::NotFound => write!(f, "Task not found."),
            TaskError::Internal => write!(f, "Internal error."),
            TaskError::AlreadyProcessed => write!(f, "Task already processed."),
            TaskError::Metadata => write!(f, "Invalid metadata."),
            TaskError::Data => write!(f, "Invalid data."),
            TaskError::ScheduleConflict => write!(f, "Schedule conflict."),
            TaskError::UserId => write!(f, "Invalid user ID."),
        }
    }
}

/// Cancels a scheduler task
pub async fn cancel_task(task_id: i64) -> Result<(), TaskError> {
    let mut pool = crate::tasks::pool::get_pool().await.ok_or_else(|| {
        tasks_error!("Couldn't get the redis pool.");
        TaskError::Internal
    })?;

    let mut task = pool.get_task_data(task_id).await.map_err(|e| {
        tasks_error!("error getting task: {}", e);
        TaskError::NotFound
    })?;

    // Can't cancel something that's already been queued
    if task.metadata.status != TaskStatus::Queued as i32 {
        return Err(TaskError::AlreadyProcessed);
    }

    task.metadata.status = TaskStatus::Rejected.into();
    task.metadata.status_rationale = Some(TaskStatusRationale::ClientCancelled.into());

    let delta = Duration::try_minutes(1).ok_or_else(|| {
        tasks_error!("error creating time delta.");
        TaskError::Internal
    })?;

    let new_expiry = Utc::now() + delta;
    pool.update_task(task_id, &task, new_expiry)
        .await
        .map_err(|e| {
            tasks_warn!("error updating task: {}", e);
            TaskError::Internal
        })?;

    Ok(())
}

/// Gets the status of a scheduler task
pub async fn get_task_status(task_id: i64) -> Result<TaskMetadata, TaskError> {
    crate::tasks::pool::get_pool()
        .await
        .ok_or_else(|| {
            tasks_error!("Couldn't get the redis pool.");
            TaskError::Internal
        })?
        .get_task_data(task_id)
        .await
        .map(|task| task.metadata)
        .map_err(|e| {
            tasks_warn!("error getting task: {}", e);
            TaskError::NotFound
        })
}

/// Iterates through priority queues and implements tasks
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) loops indefinitely
pub async fn task_loop(_config: crate::config::Config) -> Result<(), ()> {
    tasks_info!("Start.");

    let mut pool = crate::tasks::pool::get_pool().await.ok_or_else(|| {
        tasks_error!("Couldn't get the redis pool.");
    })?;

    let keepalive_delta =
        Duration::try_minutes(TASK_KEEPALIVE_DURATION_MINUTES).ok_or_else(|| {
            tasks_warn!("error creating time delta.");
        })?;

    loop {
        let (task_id, mut task) = match pool.next_task().await {
            Ok(t) => t,
            Err(_) => {
                tasks_debug!("No tasks to process, sleeping {IDLE_DURATION_MS} ms.");
                std::thread::sleep(std::time::Duration::from_millis(IDLE_DURATION_MS));
                continue;
            }
        };

        tasks_info!("Processing task: {}", task_id);

        if task.metadata.status != TaskStatus::Queued as i32 {
            // log task was already processed
            continue;
        }

        // Results of the action are stored in the task
        let result = match FromPrimitive::from_i32(task.metadata.action) {
            Some(TaskAction::CreateItinerary) => create_itinerary(&mut task).await,
            Some(TaskAction::CancelItinerary) => cancel_itinerary(&mut task).await,
            None => {
                tasks_warn!("Invalid task action: {}", task.metadata.action);
                task.metadata.status = TaskStatus::Rejected.into();
                task.metadata.status_rationale = Some(TaskStatusRationale::InvalidAction.into());
                Err(TaskError::Metadata)
            }
        };

        match result {
            Ok(_) => {
                tasks_info!("Task completed successfully.");
                task.metadata.status = TaskStatus::Complete.into();
            }
            Err(e) => {
                tasks_warn!("error executing task: {}", e);
                task.metadata.status = TaskStatus::Rejected.into();
            }
        }

        let new_expiry = Utc::now() + keepalive_delta;
        let _ = pool.update_task(task_id, &task, new_expiry).await;
    }

    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_error_display() {
        assert_eq!(TaskError::NotFound.to_string(), "Task not found.");
        assert_eq!(TaskError::Internal.to_string(), "Internal error.");
        assert_eq!(
            TaskError::AlreadyProcessed.to_string(),
            "Task already processed."
        );
        assert_eq!(TaskError::Metadata.to_string(), "Invalid metadata.");
        assert_eq!(TaskError::Data.to_string(), "Invalid data.");
        assert_eq!(
            TaskError::ScheduleConflict.to_string(),
            "Schedule conflict."
        );
        assert_eq!(TaskError::UserId.to_string(), "Invalid user ID.");
    }
}
