//! Re-export of used objects

pub use super::client as scheduler;
pub use super::service::Client as SchedulerServiceClient;

pub use lib_common::grpc::{Client, ClientConnect, GrpcClient};
pub use svc_storage_client_grpc as scheduler_storage;
pub use svc_storage_client_grpc::Timestamp;
pub use tonic::transport::Channel;
