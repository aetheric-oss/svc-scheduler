//! Redis connection pool implementation

use crate::tasks::{Task, TaskStatus};
use deadpool_redis::{
    redis::{AsyncCommands, FromRedisValue, Value},
    Pool, Runtime,
};
use lib_common::time::{DateTime, Utc};
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::sync::Arc;
use svc_storage_client_grpc::prelude::flight_plan::FlightPriority;
use tokio::sync::{Mutex, OnceCell};
use tonic::async_trait;

/// A global static Redis pool.
static REDIS_POOL: OnceCell<Arc<Mutex<TaskPool>>> = OnceCell::const_new();

/// Returns a Redis Pool.
/// Uses host and port configurations using a Config object generated from
/// environment variables.
/// Initializes the pool if it hasn't been initialized yet.
pub async fn get_pool() -> Option<TaskPool> {
    if !REDIS_POOL.initialized() {
        let config = crate::Config::try_from_env().unwrap_or_default();
        let Some(pool) = TaskPool::new(config.clone()) else {
            tasks_error!("could not create Redis pool.");
            panic!("(get_pool) could not create Redis pool.");
        };

        let value = Arc::new(Mutex::new(pool));
        if let Err(e) = REDIS_POOL.set(value) {
            tasks_error!("could not set Redis pool: {e}");
            panic!("(get_pool) could not set Redis pool: {e}");
        };
    }

    let Some(arc) = REDIS_POOL.get() else {
        tasks_error!("could not get Redis pool.");
        return None;
    };

    let pool = arc.lock().await;
    Some((*pool).clone())
}

/// Represents errors that can occur during cache operations.
#[derive(Debug, Clone, Copy)]
pub enum CacheError {
    /// Could not build configuration for cache.
    CouldNotConfigure,

    /// Could not connect to the Redis pool.
    CouldNotConnect,

    /// No tasks in the cache
    Empty,

    /// The operation on the Redis cache failed.
    OperationFailed,
}

impl Display for CacheError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            CacheError::CouldNotConfigure => write!(f, "Could not configure cache."),
            CacheError::CouldNotConnect => write!(f, "Could not connect to cache."),
            CacheError::OperationFailed => write!(f, "Cache operation failed."),
            CacheError::Empty => write!(f, "Cache is empty."),
        }
    }
}

/// Represents a pool of connections to a Redis server.
///
/// The [`TaskPool`] struct provides a managed pool of connections to a Redis server.
/// It allows clients to acquire and release connections from the pool and handles
/// connection management, such as connection pooling and reusing connections.
#[derive(Clone)]
pub struct TaskPool {
    /// The underlying pool of Redis connections.
    pool: Pool,
}

impl Debug for TaskPool {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TaskPool").finish()
    }
}

impl TaskPool {
    /// Create a new TaskPool
    pub fn new(config: crate::config::Config) -> Option<TaskPool> {
        // the .env file must have REDIS__URL="redis://\<host\>:\<port\>"
        let cfg: deadpool_redis::Config = config.redis;
        let Some(details) = cfg.url.clone() else {
            tasks_error!("(TaskPool new) no connection address found.");
            return None;
        };

        tasks_info!(
            "(TaskPool new) creating pool with key folder 'scheduler' at {:?}...",
            details
        );

        match cfg.create_pool(Some(Runtime::Tokio1)) {
            Ok(pool) => {
                tasks_info!("(TaskPool new) pool created.");
                Some(TaskPool { pool })
            }
            Err(e) => {
                tasks_error!("(TaskPool new) could not create pool: {}", e);
                None
            }
        }
    }
}

impl RedisPool for TaskPool {
    fn pool(&self) -> &Pool {
        &self.pool
    }
}

struct NextTask {
    task_id: i64,
    queue_name: String,
}

use deadpool_redis::redis::{ErrorKind, RedisError, RedisResult};
impl FromRedisValue for NextTask {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        let Value::Bulk(ref v) = v else {
            return Err(RedisError::from((
                ErrorKind::TypeError,
                "Unexpected Redis value",
            )));
        };

        let [Value::Data(ref queue_name), Value::Bulk(ref values)] = v[..] else {
            return Err(RedisError::from((
                ErrorKind::TypeError,
                "Unexpected Redis value",
            )));
        };

        let Ok(queue_name) = String::from_utf8(queue_name.to_vec()) else {
            return Err(RedisError::from((
                ErrorKind::TypeError,
                "Unexpected Redis value",
            )));
        };

        let Value::Bulk(ref values) = values[0] else {
            return Err(RedisError::from((
                ErrorKind::TypeError,
                "Unexpected Redis value",
            )));
        };

        let [Value::Data(ref task_id), _] = values[..] else {
            return Err(RedisError::from((
                ErrorKind::TypeError,
                "Unexpected Redis value",
            )));
        };

        let Ok(task_id) = String::from_utf8(task_id.to_vec()) else {
            return Err(RedisError::from((
                ErrorKind::TypeError,
                "Unexpected Redis value",
            )));
        };

        let Ok(task_id) = task_id.parse::<i64>() else {
            return Err(RedisError::from((
                ErrorKind::TypeError,
                "Unexpected Redis value",
            )));
        };

        let t = NextTask {
            task_id,
            queue_name,
        };

        RedisResult::Ok(t)
    }
}

/// Trait for interacting with a scheduler task pool
#[async_trait]
pub trait RedisPool {
    /// Returns a reference to the underlying pool.
    fn pool(&self) -> &Pool;

    /// Creates a new task and returns the task_id for it
    async fn new_task(
        &mut self,
        task: &Task,
        priority: FlightPriority,
        expiry: DateTime<Utc>,
    ) -> Result<i64, CacheError>
    where
        Self: Send + Sync + 'async_trait,
    {
        if expiry <= Utc::now() {
            tasks_error!("(RedisPool new_task) expiry must be in the future.");
            return Err(CacheError::OperationFailed);
        }

        if task.metadata.status != TaskStatus::Queued as i32 {
            tasks_error!("(RedisPool new_task) new task status must be 'Queued'.");
            return Err(CacheError::OperationFailed);
        }

        if task.metadata.status_rationale.is_some() {
            tasks_error!("(RedisPool new_task) new task status rationale must be 'None'.");
            return Err(CacheError::OperationFailed);
        }

        let queue_name = match priority {
            FlightPriority::Emergency => "scheduler:emergency",
            FlightPriority::High => "scheduler:high",
            FlightPriority::Medium => "scheduler:medium",
            FlightPriority::Low => "scheduler:low",
        };

        let Ok(expiry_ms) = TryInto::<usize>::try_into(expiry.timestamp_millis()) else {
            tasks_error!("(RedisPool new_task) Could not convert expiry into redis usize type.");
            return Err(CacheError::OperationFailed);
        };

        let mut connection = match self.pool().get().await {
            Ok(c) => c,
            Err(e) => {
                tasks_error!(
                    "(RedisPool update_task) could not get connection from pool: {}",
                    e
                );
                return Err(CacheError::OperationFailed);
            }
        };

        let counter_key = "scheduler:tasks";
        // Increment the task counter,
        //  add the task to the tasks hash, and add the task to the queue.
        //
        // TODO(R4): Update this to a transaction (atomic).
        // deadpool_redis::Pool::get() returns an aio::Connection type, which doesn't
        //  implement DerefMut. Causes issues using the transaction() function
        // let result = redis::transaction(
        //     connection.borrow_mut(),
        //     &[counter_key, queue_name],
        //     |connection, pipe| {
        //         let task_id: i64 = pipe.hincr(counter_key, "counter", 1).query(connection)?;
        //         pipe.hset(
        //             format!("{counter_key}:{task_id}"),
        //             "data".to_string(),
        //             serialized_task,
        //         )
        //         .ignore()
        //         .expire_at(format!("{counter_key}:{task_id}"), expiry_ms)
        //         .ignore()
        //         .zadd(queue_name, task_id, expiry_ms)
        //         .ignore()
        //         .query(connection);

        //         Ok(Some(task_id))
        //     },
        // );
        // let task_id = match result {
        //     Ok(t) => t,
        //     Err(e) => {
        //         tasks_error!("(RedisPool new_task) unexpected redis response: {:?}", e);
        //         return Err(CacheError::OperationFailed);
        //     }
        // };

        // Get new task ID
        let task_id: i64 = match connection.hincr(counter_key, "counter", 1).await {
            Ok(Value::Int(t)) => t,
            Ok(value) => {
                tasks_error!(
                    "(RedisPool new_task) unexpected redis response: {:?}",
                    value
                );
                return Err(CacheError::OperationFailed);
            }
            Err(e) => {
                tasks_error!("(RedisPool new_task) unexpected redis response: {:?}", e);
                return Err(CacheError::OperationFailed);
            }
        };

        // Add task to tasks hash
        let key = format!("{counter_key}:{task_id}");
        match connection.hset(key.clone(), "data".to_string(), task).await {
            Ok(Value::Int(1)) => (),
            Ok(value) => {
                tasks_error!(
                    "(RedisPool new_task) unexpected redis response: {:?}",
                    value
                );
                return Err(CacheError::OperationFailed);
            }
            Err(e) => {
                tasks_error!("(RedisPool new_task) could not set task #{task_id} data: {e}",);
                return Err(CacheError::OperationFailed);
            }
        };

        // Add expiration to task
        // Currently shouldn't cause to fail, but may in the future
        match connection.expire_at(key.clone(), expiry_ms).await {
            Ok(Value::Int(1)) => (),
            Ok(value) => {
                tasks_error!(
                    "(RedisPool new_task) unexpected redis response: {:?}",
                    value
                );
            }
            Err(e) => {
                tasks_error!("(RedisPool new_task) could not set task #{task_id} expiry: {e}",);
            }
        };

        // Add task to queue
        match connection
            .zadd(queue_name.to_string(), task_id, expiry_ms)
            .await
        {
            Ok(Value::Int(1)) => (),
            Ok(value) => {
                tasks_error!(
                    "(RedisPool new_task) unexpected redis response: {:?}",
                    value
                );
                return Err(CacheError::OperationFailed);
            }
            Err(e) => {
                tasks_error!("(RedisPool new_task) could not set task #{task_id} data: {e}",);
                return Err(CacheError::OperationFailed);
            }
        };

        tasks_info!("(RedisPool new_task) created new task #{task_id} in '{queue_name}' queue.",);
        tasks_debug!("(RedisPool new_task) new task #{task_id} data: {:?}", task);

        Ok(task_id)
    }

    /// Updates task information
    async fn update_task(
        &mut self,
        task_id: i64,
        task: &Task,
        expiry: DateTime<Utc>,
    ) -> Result<(), CacheError>
    where
        Self: Send + Sync + 'async_trait,
    {
        let key = format!("scheduler:tasks:{}", task_id);
        let mut connection = match self.pool().get().await {
            Ok(c) => c,
            Err(e) => {
                tasks_error!(
                    "(RedisPool update_task) could not get connection from pool: {}",
                    e
                );
                return Err(CacheError::OperationFailed);
            }
        };

        let Ok(expiry_ms) = TryInto::<usize>::try_into(expiry.timestamp_millis()) else {
            tasks_error!("(RedisPool update_task) Could not convert expiry into redis usize type.");
            return Err(CacheError::OperationFailed);
        };

        //
        // If this is used on a nonexistent key, it will NOT create a task
        //
        // TODO(R4): Use a transaction
        //  Currently has issues as deadpool_redis::Pool::get() returns an aio::Connection type,
        //  which doesn't implement DerefMut
        // let result = redis::transaction(&mut con, &[key], |con, pipe| {
        //     if !con.hexists(key, "data")? {
        //         tasks_error!(
        //             "(RedisPool update_task) task with id {} does not exist.",
        //             task_id
        //         );
        //         return Ok(None);
        //     }
        //     pipe.hset(key, "data".to_string(), serialized_task)
        //         .ignore()
        //         .expire_at(key, expiry_ms)
        //         .ignore()
        //         .query(con)
        // });
        // match result {
        //     Ok(Some(_)) => {
        //         tasks_info!("(RedisPool update_task) updated task #{task_id}.");
        //         tasks_debug!(
        //             "(RedisPool update_task) updated task #{task_id} data: {:?}",
        //             task
        //         );
        //         Ok(())
        //     }
        //     Ok(None) => {
        //         tasks_error!(
        //             "(RedisPool update_task) task with id {} does not exist.",
        //             task_id
        //         );
        //         Err(CacheError::OperationFailed)
        //     }
        //     Err(e) => {
        //         tasks_error!("(RedisPool update_task) unexpected redis response: {e}");
        //         Err(CacheError::OperationFailed)
        //     }
        // }

        match connection.hset(key.clone(), "data".to_string(), task).await {
            Ok(Value::Int(0)) => (), // zero new fields added in update
            Ok(value) => {
                tasks_error!(
                    "(RedisPool update_task) unexpected redis response: {:?}",
                    value
                );
                return Err(CacheError::OperationFailed);
            }
            Err(e) => {
                tasks_error!("(RedisPool update_task) could not set task #{task_id} data: {e}",);
                return Err(CacheError::OperationFailed);
            }
        };

        match connection.expire_at(key.clone(), expiry_ms).await {
            Ok(Value::Int(1)) => (),
            Ok(value) => {
                tasks_error!(
                    "(RedisPool new_task) unexpected redis response: {:?}",
                    value
                );
            }
            Err(e) => {
                tasks_error!("(RedisPool new_task) could not set task #{task_id} expiry: {e}",);
            }
        };

        Ok(())
    }

    /// Gets task information
    async fn get_task_data(&mut self, task_id: i64) -> Result<Task, CacheError>
    where
        Self: Send + Sync + 'async_trait,
    {
        let key = format!("scheduler:tasks:{}", task_id);
        let mut connection = match self.pool().get().await {
            Ok(c) => c,
            Err(e) => {
                tasks_error!(
                    "(RedisPool update_task) could not get connection from pool: {}",
                    e
                );
                return Err(CacheError::OperationFailed);
            }
        };

        match connection.hget(key, "data".to_string()).await {
            Ok(result) => Ok(result),
            Err(e) => {
                tasks_error!(
                    "(RedisPool get_task_data) unexpected redis response: {:?}",
                    e
                );
                Err(CacheError::OperationFailed)
            }
        }
    }

    /// Updates task information
    async fn next_task(&mut self) -> Result<(i64, Task), CacheError>
    where
        Self: Send + Sync + 'async_trait,
    {
        let counter_key = "scheduler:tasks";
        let queues = vec![
            "scheduler:emergency",
            "scheduler:high",
            "scheduler:medium",
            "scheduler:low",
        ];

        let mut keys = queues.clone();
        keys.push(counter_key);

        let mut connection = match self.pool().get().await {
            Ok(c) => c,
            Err(e) => {
                tasks_error!(
                    "(RedisPool update_task) could not get connection from pool: {}",
                    e
                );
                return Err(CacheError::OperationFailed);
            }
        };

        // TODO(R4): Make this section a transaction if possible
        // DerefMut is currently not implemented for aio::Connection type returned
        //  by deadpool_redis::Pool::get()
        let (task_id, queue_name) = match connection.zmpop_min(&queues, 1).await {
            Ok(Value::Nil) => {
                tasks_debug!("(RedisPool next_task) no tasks in queues.");
                return Err(CacheError::Empty);
            }
            Ok(Value::Bulk(b)) => {
                let Ok(next_task) = NextTask::from_redis_value(&Value::Bulk(b.clone())) else {
                    tasks_debug!("(RedisPool next_task) unexpected redis response: {:?}", b);
                    return Err(CacheError::OperationFailed);
                };

                (next_task.task_id, next_task.queue_name)
            }
            Ok(value) => {
                tasks_debug!(
                    "(RedisPool next_task) unexpected redis response: {:?}",
                    value
                );
                return Err(CacheError::OperationFailed);
            }
            Err(e) => {
                tasks_debug!("(RedisPool next_task) could not pop task from queue: {e}");
                return Err(CacheError::OperationFailed);
            }
        };

        let key = format!("{counter_key}:{task_id}");
        let task = match connection.hget(key, "data".to_string()).await {
            Ok(t) => t,
            Err(e) => {
                tasks_error!("(RedisPool next_task) could not get task #{task_id} from hash: {e}",);

                return Err(CacheError::OperationFailed);
            }
        };
        //
        // End Transaction Section
        //

        tasks_info!(
            "(RedisPool next_task) popped task #{task_id} from {}.",
            queue_name.to_string()
        );
        tasks_debug!("(RedisPool next_task) task #{task_id} data: {:?}", task);

        Ok((task_id, task))
    }
}
