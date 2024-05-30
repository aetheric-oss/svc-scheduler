//! Client Library: Client Functions, Structs, Traits
#![allow(unused_qualifications)]
include!("grpc.rs");

use super::*;
use lib_common::uuid::Uuid;

#[cfg(not(feature = "stub_client"))]
use lib_common::grpc::ClientConnect;
use lib_common::grpc::{Client, GrpcClient};
use rpc_service_client::RpcServiceClient;
/// GrpcClient implementation of the RpcServiceClient
pub type SchedulerClient = GrpcClient<RpcServiceClient<Channel>>;

cfg_if::cfg_if! {
    if #[cfg(feature = "stub_backends")] {
        use svc_scheduler::grpc::server::{RpcServiceServer, ServerImpl};
        lib_common::grpc_mock_client!(RpcServiceClient, RpcServiceServer, ServerImpl);
        super::log_macros!("grpc", "app::client::mock::scheduler");
    } else {
        lib_common::grpc_client!(RpcServiceClient);
        super::log_macros!("grpc", "app::client::scheduler");
    }
}

#[cfg(feature = "stub_client")]
use rand::Rng;

#[cfg(not(feature = "stub_client"))]
#[async_trait]
impl crate::service::Client<RpcServiceClient<Channel>> for SchedulerClient {
    type ReadyRequest = ReadyRequest;
    type ReadyResponse = ReadyResponse;

    async fn is_ready(
        &self,
        request: Self::ReadyRequest,
    ) -> Result<tonic::Response<Self::ReadyResponse>, tonic::Status> {
        grpc_info!("{} client.", self.get_name());
        grpc_debug!("request: {:?}", request);
        let mut client = self.get_client().await?;
        client.is_ready(request).await
    }

    async fn query_flight(
        &self,
        request: QueryFlightRequest,
    ) -> Result<tonic::Response<QueryFlightResponse>, tonic::Status> {
        grpc_info!("{} client.", self.get_name());
        grpc_debug!("request: {:?}", request);
        let mut client = self.get_client().await?;
        client.query_flight(request).await
    }

    async fn create_itinerary(
        &self,
        request: CreateItineraryRequest,
    ) -> Result<tonic::Response<TaskResponse>, tonic::Status> {
        grpc_info!("{} client.", self.get_name());
        grpc_debug!("request: {:?}", request);
        let mut client = self.get_client().await?;
        client.create_itinerary(request).await
    }

    async fn cancel_itinerary(
        &self,
        request: CancelItineraryRequest,
    ) -> Result<tonic::Response<TaskResponse>, tonic::Status> {
        grpc_info!("{} client.", self.get_name());
        grpc_debug!("request: {:?}", request);
        let mut client = self.get_client().await?;
        client.cancel_itinerary(request).await
    }

    async fn cancel_task(
        &self,
        request: TaskRequest,
    ) -> Result<tonic::Response<TaskResponse>, tonic::Status> {
        grpc_info!("{} client.", self.get_name());
        grpc_debug!("request: {:?}", request);
        let mut client = self.get_client().await?;
        client.cancel_task(request).await
    }

    async fn get_task_status(
        &self,
        request: TaskRequest,
    ) -> Result<tonic::Response<TaskResponse>, tonic::Status> {
        grpc_info!("{} client.", self.get_name());
        grpc_debug!("request: {:?}", request);
        let mut client = self.get_client().await?;
        client.get_task_status(request).await
    }
}

#[cfg(feature = "stub_client")]
#[async_trait]
impl crate::service::Client<RpcServiceClient<Channel>> for SchedulerClient {
    type ReadyRequest = ReadyRequest;
    type ReadyResponse = ReadyResponse;

    async fn is_ready(
        &self,
        request: Self::ReadyRequest,
    ) -> Result<tonic::Response<Self::ReadyResponse>, tonic::Status> {
        grpc_warn!("(MOCK) {} client.", self.get_name());
        grpc_debug!("(MOCK) request: {:?}", request);
        Ok(tonic::Response::new(ReadyResponse { ready: true }))
    }

    async fn query_flight(
        &self,
        request: QueryFlightRequest,
    ) -> Result<tonic::Response<QueryFlightResponse>, tonic::Status> {
        grpc_warn!("(MOCK) {} client.", self.get_name());
        grpc_debug!("(MOCK) request: {:?}", request);
        let flight_plan_data = prelude::scheduler_storage::flight_plan::mock::get_future_data_obj();
        let itineraries = vec![Itinerary {
            flight_plans: vec![flight_plan_data],
        }];

        Ok(tonic::Response::new(QueryFlightResponse { itineraries }))
    }

    async fn create_itinerary(
        &self,
        request: CreateItineraryRequest,
    ) -> Result<tonic::Response<TaskResponse>, tonic::Status> {
        grpc_warn!("(MOCK) {} client.", self.get_name());
        grpc_debug!("(MOCK) request: {:?}", request);
        let mut rng = rand::thread_rng();
        Ok(tonic::Response::new(TaskResponse {
            task_id: rng.gen_range(0..1000000),
            task_metadata: Some(TaskMetadata {
                status: TaskStatus::Queued.into(),
                status_rationale: None,
                action: TaskAction::CreateItinerary.into(),
                user_id: request.user_id,
                result: None,
            }),
        }))
    }

    async fn cancel_itinerary(
        &self,
        request: CancelItineraryRequest,
    ) -> Result<tonic::Response<TaskResponse>, tonic::Status> {
        grpc_info!("(MOCK) {} client.", self.get_name());
        grpc_debug!("(MOCK) request: {:?}", request);
        let mut rng = rand::thread_rng();
        Ok(tonic::Response::new(TaskResponse {
            task_id: rng.gen_range(0..1000000),
            task_metadata: Some(TaskMetadata {
                status: TaskStatus::Queued.into(),
                status_rationale: None,
                action: TaskAction::CancelItinerary.into(),
                user_id: request.user_id,
                result: None,
            }),
        }))
    }

    async fn cancel_task(
        &self,
        request: TaskRequest,
    ) -> Result<tonic::Response<TaskResponse>, tonic::Status> {
        grpc_info!("(MOCK) {} client.", self.get_name());
        grpc_debug!("(MOCK) request: {:?}", request);
        Ok(tonic::Response::new(TaskResponse {
            task_id: request.task_id,
            task_metadata: Some(TaskMetadata {
                status: TaskStatus::Rejected.into(),
                status_rationale: Some(TaskStatusRationale::ClientCancelled.into()),
                action: TaskAction::CancelItinerary.into(),
                user_id: Uuid::new_v4().to_string(), // arbitrary
                result: None,
            }),
        }))
    }

    async fn get_task_status(
        &self,
        request: TaskRequest,
    ) -> Result<tonic::Response<TaskResponse>, tonic::Status> {
        grpc_info!("(MOCK) {} client.", self.get_name());
        grpc_debug!("(MOCK) request: {:?}", request);
        Ok(tonic::Response::new(TaskResponse {
            task_id: request.task_id,
            task_metadata: Some(TaskMetadata {
                status: TaskStatus::Complete.into(),
                status_rationale: None,
                action: TaskAction::CreateItinerary.into(),
                user_id: Uuid::new_v4().to_string(), // arbitrary
                result: None,
            }),
        }))
    }
}

#[cfg(test)]
mod tests {
    use crate::service::Client as ServiceClient;

    use super::*;

    #[tokio::test]
    #[cfg(not(feature = "stub_client"))]
    async fn test_client_connect() {
        let name = "scheduler";
        let (server_host, server_port) =
            lib_common::grpc::get_endpoint_from_env("GRPC_HOST", "GRPC_PORT");

        let client: SchedulerClient = GrpcClient::new_client(&server_host, server_port, name);
        assert_eq!(client.get_name(), name);

        let connection = client.get_client().await;
        println!("{:?}", connection);
        assert!(connection.is_ok());
    }

    #[tokio::test]
    async fn test_client_is_ready_request() {
        let name = "scheduler";
        let (server_host, server_port) =
            lib_common::grpc::get_endpoint_from_env("GRPC_HOST", "GRPC_PORT");

        let client: SchedulerClient = GrpcClient::new_client(&server_host, server_port, name);
        assert_eq!(client.get_name(), name);

        let result = client.is_ready(ReadyRequest {}).await;
        println!("{:?}", result);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().into_inner().ready, true);
    }
}
