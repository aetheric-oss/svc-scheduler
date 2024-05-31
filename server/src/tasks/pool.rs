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
        let Ok(config) = crate::Config::try_from_env() else {
            tasks_error!("could not build configuration for cache.");
            panic!("(get_pool) could not build configuration for cache.");
        };

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
#[derive(Debug, Clone, Copy, PartialEq)]
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

#[derive(Debug)]
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
                "Unexpected Redis value, not a Bulk type",
            )));
        };

        let [Value::Data(ref queue_name), Value::Bulk(ref values)] = v[..] else {
            return Err(RedisError::from((
                ErrorKind::TypeError,
                "Unexpected Redis values, not a Bulk of [Data, Bulk]",
            )));
        };

        let queue_name = String::from_utf8(queue_name.to_vec()).map_err(|_| {
            RedisError::from((ErrorKind::TypeError, "Invalid queue name, non UTF-8."))
        })?;

        let Value::Bulk(ref values) = values[0] else {
            return Err(RedisError::from((
                ErrorKind::TypeError,
                "Unexpected values, first Bulk value not a Bulk type",
            )));
        };

        let [Value::Data(ref task_id), _] = values[..] else {
            return Err(RedisError::from((
                ErrorKind::TypeError,
                "Unexpected values, first member not a Data type",
            )));
        };

        let task_id = String::from_utf8(task_id.to_vec())
            .map_err(|_| {
                RedisError::from((ErrorKind::TypeError, "Could not parse task_id to String."))
            })?
            .parse::<i64>()
            .map_err(|_| {
                RedisError::from((ErrorKind::TypeError, "Could not parse task_id to i64."))
            })?;

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

    /// Validate a new task
    /// Separated for easier unit testing
    fn new_task_validation(task: &Task, expiry: DateTime<Utc>) -> Result<(), CacheError> {
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

        Ok(())
    }

    /// Creates a new task and returns the task_id for it
    #[cfg(not(tarpaulin_include))]
    // no_coverage: (R5) need redis backend to test this
    async fn new_task(
        &mut self,
        task: &Task,
        priority: FlightPriority,
        expiry: DateTime<Utc>,
    ) -> Result<i64, CacheError>
    where
        Self: Send + Sync + 'async_trait,
    {
        Self::new_task_validation(task, expiry)?;

        let queue_name = match priority {
            FlightPriority::Emergency => "scheduler:emergency",
            FlightPriority::High => "scheduler:high",
            FlightPriority::Medium => "scheduler:medium",
            FlightPriority::Low => "scheduler:low",
        };

        let expiry_ms = TryInto::<usize>::try_into(expiry.timestamp_millis()).map_err(|e| {
            tasks_error!(
                "(RedisPool new_task) Could not convert expiry into redis usize type: {e}"
            );
            CacheError::OperationFailed
        })?;

        let mut connection = self.pool().get().await.map_err(|e| {
            tasks_error!(
                "(RedisPool update_task) could not get connection from pool: {}",
                e
            );

            CacheError::OperationFailed
        })?;

        let counter_key = "scheduler:tasks";

        // Increment the task counter,
        //  add the task to the tasks hash, and add the task to the queue.
        //
        // TODO(R5): Update this to a transaction (atomic).
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
        let response = connection
            .hincr(counter_key, "counter", 1)
            .await
            .map_err(|e| {
                tasks_error!("(RedisPool new_task) unexpected redis response: {:?}", e);

                CacheError::OperationFailed
            })?;

        let Value::Int(task_id) = response else {
            tasks_error!(
                "(RedisPool new_task) unexpected redis response: {:?}",
                response
            );

            return Err(CacheError::OperationFailed);
        };

        // Add task to tasks hash
        let key = format!("{counter_key}:{task_id}");
        let Value::Int(1) = connection
            .hset(key.clone(), "data".to_string(), task)
            .await
            .map_err(|e| {
                tasks_error!("(RedisPool new_task) could not set task #{task_id} data: {e}",);

                CacheError::OperationFailed
            })?
        else {
            tasks_error!(
                "(RedisPool new_task) unexpected redis response: {:?}",
                response
            );

            return Err(CacheError::OperationFailed);
        };

        // Add expiration to task
        // TODO(R6): cleanup job for tasks that don't have an expiry date
        let response = connection
            .expire_at(key.clone(), expiry_ms)
            .await
            .map_err(|e| {
                tasks_error!("(RedisPool new_task) could not set task #{task_id} expiry: {e}",);
                CacheError::OperationFailed
            })?;

        match response {
            Value::Int(1) => (),
            value => {
                tasks_error!(
                    "(RedisPool new_task) unexpected redis response: {:?}",
                    value
                );

                return Err(CacheError::OperationFailed);
            }
        };

        // Add task to queue
        let response = connection
            .zadd(queue_name.to_string(), task_id, expiry_ms)
            .await
            .map_err(|e| {
                tasks_error!(
                    "(RedisPool new_task) could not add task #{task_id} to '{queue_name}' queue: {e}",
                );

                CacheError::OperationFailed
            })?;

        let Value::Int(1) = response else {
            tasks_error!(
                "(RedisPool new_task) unexpected redis response: {:?}",
                response
            );

            return Err(CacheError::OperationFailed);
        };

        tasks_info!("(RedisPool new_task) created new task #{task_id} in '{queue_name}' queue.",);
        tasks_debug!("(RedisPool new_task) new task #{task_id} data: {:?}", task);

        Ok(task_id)
    }

    /// Updates task information
    #[cfg(not(tarpaulin_include))]
    // no_coverage: (R5) need redis backend to test this
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
        let mut connection = self.pool().get().await.map_err(|e| {
            tasks_error!(
                "(RedisPool update_task) could not get connection from pool: {}",
                e
            );
            CacheError::OperationFailed
        })?;

        let expiry_ms = TryInto::<usize>::try_into(expiry.timestamp_millis()).map_err(|_| {
            tasks_error!("(RedisPool update_task) Could not convert expiry into redis usize type.");
            CacheError::OperationFailed
        })?;

        //
        // If this is used on a nonexistent key, it will NOT create a task
        //
        // TODO(R5): Use a transaction
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

        let response = connection
            .hset(key.clone(), "data".to_string(), task)
            .await
            .map_err(|e| {
                tasks_error!("(RedisPool update_task) could not set task #{task_id} data: {e}",);
                CacheError::OperationFailed
            })?;

        // expect zero new fields added in update
        let Value::Int(0) = response else {
            tasks_error!(
                "(RedisPool update_task) unexpected redis response: {:?}",
                response
            );

            return Err(CacheError::OperationFailed);
        };

        // don't fail if expiry can't be changed, this task already exists
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
    #[cfg(not(tarpaulin_include))]
    // no_coverage: (R5) need redis backend to test this
    async fn get_task_data(&mut self, task_id: i64) -> Result<Task, CacheError>
    where
        Self: Send + Sync + 'async_trait,
    {
        let key = format!("scheduler:tasks:{}", task_id);

        self.pool()
            .get()
            .await
            .map_err(|e| {
                tasks_error!(
                    "(RedisPool update_task) could not get connection from pool: {}",
                    e
                );

                CacheError::OperationFailed
            })?
            .hget(key, "data".to_string())
            .await
            .map_err(|e| {
                tasks_error!(
                    "(RedisPool get_task_data) could not get task #{task_id} from hash: {e}",
                );

                CacheError::OperationFailed
            })
    }

    /// Updates task information
    #[cfg(not(tarpaulin_include))]
    // no_coverage: (R5) need redis backend to test this
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

        let mut connection = self.pool().get().await.map_err(|e| {
            tasks_error!("(RedisPool update_task) could not get connection from pool: {e}");
            CacheError::OperationFailed
        })?;

        // TODO(R5): Make this section a transaction if possible
        // DerefMut is currently not implemented for aio::Connection type returned
        //  by deadpool_redis::Pool::get()
        let response = connection.zmpop_min(&queues, 1).await.map_err(|e| {
            tasks_error!("(RedisPool next_task) could not pop task from queue: {e}",);

            CacheError::OperationFailed
        })?;

        let (task_id, queue_name) = match response {
            Value::Nil => {
                tasks_debug!("(RedisPool next_task) no tasks in queues.");
                return Err(CacheError::Empty);
            }
            Value::Bulk(b) => {
                let next_task =
                    NextTask::from_redis_value(&Value::Bulk(b.clone())).map_err(|e| {
                        tasks_debug!(
                            "(RedisPool next_task) unexpected redis response: {:?}; {e}",
                            b
                        );
                        CacheError::OperationFailed
                    })?;

                (next_task.task_id, next_task.queue_name)
            }
            value => {
                tasks_debug!(
                    "(RedisPool next_task) unexpected redis response: {:?}",
                    value
                );

                return Err(CacheError::OperationFailed);
            }
        };

        let key = format!("{counter_key}:{task_id}");
        let task = connection
            .hget(key, "data".to_string())
            .await
            .map_err(|e| {
                tasks_error!("(RedisPool next_task) could not get task #{task_id} from hash: {e}",);

                CacheError::OperationFailed
            })?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{TaskAction, TaskBody, TaskMetadata, TaskStatus, TaskStatusRationale};
    use lib_common::time::Duration;
    use lib_common::uuid::Uuid;

    #[test]
    fn test_cache_error_display() {
        assert_eq!(
            format!("{}", CacheError::CouldNotConfigure),
            "Could not configure cache."
        );
        assert_eq!(
            format!("{}", CacheError::CouldNotConnect),
            "Could not connect to cache."
        );
        assert_eq!(
            format!("{}", CacheError::OperationFailed),
            "Cache operation failed."
        );
        assert_eq!(format!("{}", CacheError::Empty), "Cache is empty.");
    }

    #[test]
    fn test_debug_task_pool() {
        let mut config = crate::config::Config::default();
        config.redis.url = Some("redis://localhost:6379".to_string());

        let pool = TaskPool::new(config.clone()).unwrap();
        assert_eq!(format!("{:?}", pool), "TaskPool");
    }

    #[test]
    fn test_new_task_pool() {
        let mut config = crate::config::Config::default();

        // no address
        config.redis.url = None;
        assert!(TaskPool::new(config.clone()).is_none());

        // nonsense address
        config.redis.url = Some("r??>>>>M<MM".to_string());
        assert!(TaskPool::new(config.clone()).is_none());

        // some address
        config.redis.url = Some("redis://localhost:6379".to_string());
        assert!(TaskPool::new(config.clone()).is_some());
    }

    #[tokio::test]
    async fn test_redis_pool_new_task() {
        let mut config = crate::config::Config::default();
        config.redis.url = Some("redis://localhost:6379".to_string());
        let mut pool = TaskPool::new(config.clone()).unwrap();

        // <= Utc::now()
        let task = Task {
            metadata: TaskMetadata {
                status: TaskStatus::Queued as i32,
                status_rationale: None,
                action: TaskAction::CancelItinerary as i32,
                user_id: Uuid::new_v4().to_string(),
                result: None,
            },
            body: TaskBody::CancelItinerary(Uuid::new_v4()),
        };
        let error = pool
            .new_task(&task, FlightPriority::Emergency, Utc::now())
            .await
            .unwrap_err();
        assert_eq!(error, CacheError::OperationFailed);

        // status != TaskStatus::Queued as i32
        let mut tmp = task.clone();
        tmp.metadata.status = TaskStatus::Complete as i32;
        let error = pool
            .new_task(
                &tmp,
                FlightPriority::Emergency,
                Utc::now() + Duration::days(1),
            )
            .await
            .unwrap_err();
        assert_eq!(error, CacheError::OperationFailed);

        // status_rationale.is_some()
        let mut tmp = task.clone();
        tmp.metadata.status_rationale = Some(TaskStatusRationale::InvalidAction as i32);
        let error = pool
            .new_task(
                &tmp,
                FlightPriority::Emergency,
                Utc::now() + Duration::days(1),
            )
            .await
            .unwrap_err();
        assert_eq!(error, CacheError::OperationFailed);
    }

    #[test]
    fn test_next_task_from_redis_value() {
        let value = Value::Bulk(vec![
            Value::Data("scheduler:emergency".as_bytes().to_vec()),
            Value::Bulk(vec![Value::Bulk(vec![
                Value::Data("1".as_bytes().to_vec()),
                Value::Data("task data".as_bytes().to_vec()),
            ])]),
        ]);

        let next_task = NextTask::from_redis_value(&value).unwrap();
        assert_eq!(next_task.task_id, 1);
        assert_eq!(next_task.queue_name, "scheduler:emergency");

        // not expected type
        let value = Value::Int(1);
        let error = NextTask::from_redis_value(&value).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::TypeError);

        // bulk doesn't contain expected types
        let value = Value::Bulk(vec![Value::Data("scheduler:emergency".as_bytes().to_vec())]);
        let error = NextTask::from_redis_value(&value).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::TypeError);

        // invalid queue name, not UTF-8
        let value = Value::Bulk(vec![Value::Data(vec![0xFF]), Value::Bulk(vec![])]);
        let error = NextTask::from_redis_value(&value).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::TypeError);
        assert_eq!(
            error.to_string(),
            "Invalid queue name, non UTF-8.- TypeError"
        );

        // invalid task_id, not UTF-8
        let value = Value::Bulk(vec![
            Value::Data("scheduler:emergency".as_bytes().to_vec()),
            Value::Bulk(vec![Value::Bulk(vec![
                Value::Data(vec![0xFF]),
                Value::Data("task data".as_bytes().to_vec()),
            ])]),
        ]);
        let error = NextTask::from_redis_value(&value).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::TypeError);
        assert_eq!(
            error.to_string(),
            "Could not parse task_id to String.- TypeError"
        );

        // invalid task_id, not able to parse to i64
        let value = Value::Bulk(vec![
            Value::Data("scheduler:emergency".as_bytes().to_vec()),
            Value::Bulk(vec![Value::Bulk(vec![
                Value::Data("not a number".as_bytes().to_vec()),
                Value::Data("task data".as_bytes().to_vec()),
            ])]),
        ]);
        let error = NextTask::from_redis_value(&value).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::TypeError);
        assert_eq!(
            error.to_string(),
            "Could not parse task_id to i64.- TypeError"
        );

        // first Bulk value not a Bulk type
        let value = Value::Bulk(vec![
            Value::Data("scheduler:emergency".as_bytes().to_vec()),
            Value::Bulk(vec![Value::Data("not a bulk".as_bytes().to_vec())]),
        ]);
        let error = NextTask::from_redis_value(&value).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::TypeError);
        assert_eq!(
            error.to_string(),
            "Unexpected values, first Bulk value not a Bulk type- TypeError"
        );

        // not a data type
        let value = Value::Bulk(vec![
            Value::Data("scheduler:emergency".as_bytes().to_vec()),
            Value::Bulk(vec![Value::Bulk(vec![
                Value::Bulk(vec![]), // not a data type
            ])]),
        ]);
        let error = NextTask::from_redis_value(&value).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::TypeError);
        assert_eq!(
            error.to_string(),
            "Unexpected values, first member not a Data type- TypeError"
        );
    }
}
