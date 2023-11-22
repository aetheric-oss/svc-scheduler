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
use chrono::{Duration, Utc};
use deadpool_redis::redis::{self, FromRedisValue, ToRedisArgs};
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Result as FmtResult};

/// How long to keep a task in memory after it's been processed
const TASK_KEEPALIVE_DURATION_MINUTES: i64 = 60;
/// How long to sleep (in milliseconds) if the queue is empty
const IDLE_DURATION_MS: u64 = 1000;

/// The required information to complete a task
#[derive(Serialize, Deserialize, Debug)]
pub enum TaskBody {
    /// Cancel an itinerary
    CancelItinerary(uuid::Uuid),

    /// Create an itinerary
    CreateItinerary(Vec<FlightPlanSchedule>),
}

/// Complete information about a task
#[derive(Serialize, Deserialize, Debug)]
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
            tasks_warn!("(ToRedisArgs) error serializing task");
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
    InvalidMetadata,

    /// Invalid data provided
    InvalidData,

    /// Schedule Conflict
    ScheduleConflict,
}

impl Display for TaskError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            TaskError::NotFound => write!(f, "Task not found."),
            TaskError::Internal => write!(f, "Internal error."),
            TaskError::AlreadyProcessed => write!(f, "Task already processed."),
            TaskError::InvalidMetadata => write!(f, "Invalid metadata."),
            TaskError::InvalidData => write!(f, "Invalid data."),
            TaskError::ScheduleConflict => write!(f, "Schedule conflict."),
        }
    }
}

/// Cancels a scheduler task
pub async fn cancel_task(task_id: i64) -> Result<(), TaskError> {
    let Some(mut pool) = crate::tasks::pool::get_pool().await else {
        tasks_error!("(cancel_task) Couldn't get the redis pool.");
        return Err(TaskError::Internal);
    };

    let Ok(mut task) = pool.get_task_data(task_id).await else {
        return Err(TaskError::NotFound);
    };

    // Can't cancel something that's already been queued
    if task.metadata.status != TaskStatus::Queued as i32 {
        return Err(TaskError::AlreadyProcessed);
    }

    task.metadata.status = TaskStatus::Rejected.into();
    task.metadata.status_rationale = Some(TaskStatusRationale::ClientCancelled.into());

    let new_expiry = Utc::now() + Duration::minutes(TASK_KEEPALIVE_DURATION_MINUTES);
    if let Err(e) = pool.update_task(task_id, &task, new_expiry).await {
        tasks_warn!("(cancel_task) error updating task: {}", e);
        return Err(TaskError::Internal);
    }

    Ok(())
}

/// Gets the status of a scheduler task
pub async fn get_task_status(task_id: i64) -> Result<TaskMetadata, TaskError> {
    let Some(mut pool) = crate::tasks::pool::get_pool().await else {
        tasks_error!("(get_task_status) Couldn't get the redis pool.");
        return Err(TaskError::Internal);
    };

    match pool.get_task_data(task_id).await {
        Ok(task) => Ok(task.metadata),
        Err(e) => {
            tasks_warn!("(get_task_status) error getting task: {}", e);
            Err(TaskError::NotFound)
        }
    }
}

/// Iterates through priority queues and implements tasks
pub async fn task_loop(_config: crate::config::Config) {
    tasks_info!("(task_loop) Start.");

    let Some(mut pool) = crate::tasks::pool::get_pool().await else {
        tasks_error!("(task_loop) Couldn't get the redis pool.");
        return;
    };

    loop {
        let (task_id, mut task) = match pool.next_task().await {
            Ok(t) => t,
            Err(_) => {
                tasks_debug!("(task_loop) No tasks to process, sleeping {IDLE_DURATION_MS} ms.");
                std::thread::sleep(std::time::Duration::from_millis(IDLE_DURATION_MS));
                continue;
            }
        };

        tasks_info!("(task_loop) Processing task: {}", task_id);

        if task.metadata.status != TaskStatus::Queued as i32 {
            // log task was already processed
            continue;
        }

        // Results of the action are stored in the task
        let result = match FromPrimitive::from_i32(task.metadata.action) {
            Some(TaskAction::CreateItinerary) => create_itinerary(&mut task).await,
            Some(TaskAction::CancelItinerary) => cancel_itinerary(&mut task).await,
            None => {
                tasks_warn!("(task_loop) Invalid task action: {}", task.metadata.action);

                task.metadata.status = TaskStatus::Rejected.into();
                task.metadata.status_rationale = Some(TaskStatusRationale::InvalidAction.into());
                Ok(())
            }
        };

        if let Err(e) = result {
            tasks_warn!("(task_loop) error executing task: {}", e);
            task.metadata.status = TaskStatus::Rejected.into();
            task.metadata.status_rationale = Some(TaskStatusRationale::Internal.into());
        }

        let new_expiry = Utc::now() + Duration::minutes(TASK_KEEPALIVE_DURATION_MINUTES);
        let _ = pool.update_task(task_id, &task, new_expiry).await;
    }
}
