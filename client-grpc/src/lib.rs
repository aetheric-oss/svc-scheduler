#![doc = include_str!("../README.md")]

pub mod service;

pub use client::*;
pub use lib_common::grpc::{Client, ClientConnect, GrpcClient};

use lib_common::log_macros;
use tonic::async_trait;
use tonic::transport::Channel;

pub mod client {
    //! Client Library: Client Functions, Structs, Traits
    #![allow(unused_qualifications)]
    include!("grpc.rs");

    use super::*;

    pub use rpc_service_client::RpcServiceClient;
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
}

#[cfg(not(feature = "stub_client"))]
#[async_trait]
impl crate::service::Client<RpcServiceClient<Channel>> for GrpcClient<RpcServiceClient<Channel>> {
    type ReadyRequest = ReadyRequest;
    type ReadyResponse = ReadyResponse;

    async fn is_ready(
        &self,
        request: Self::ReadyRequest,
    ) -> Result<tonic::Response<Self::ReadyResponse>, tonic::Status> {
        grpc_info!("(is_ready) {} client.", self.get_name());
        grpc_debug!("(is_ready) request: {:?}", request);
        let mut client = self.get_client().await?;
        let mut tries = 0;
        while client.is_ready(ReadyRequest {}).await.is_err() && tries < 5 {
            grpc_info!(
                "(is_ready) Client not ready yet at attempt {}, retry in 1 sec...",
                tries
            );
            tokio::time::sleep(tokio::time::Duration::new(2, 0)).await;
            tries += 1;
        }
        client.is_ready(request).await
    }

    async fn query_flight(
        &self,
        request: QueryFlightRequest,
    ) -> Result<tonic::Response<QueryFlightResponse>, tonic::Status> {
        grpc_info!("(query_flight) {} client.", self.get_name());
        grpc_debug!("(query_flight) request: {:?}", request);
        let mut client = self.get_client().await?;
        let mut tries = 0;
        while client.is_ready(ReadyRequest {}).await.is_err() && tries < 5 {
            grpc_info!(
                "(query_flight) Client not ready yet at attempt {}, retry in 1 sec...",
                tries
            );
            tokio::time::sleep(tokio::time::Duration::new(2, 0)).await;
            tries += 1;
        }
        client.query_flight(request).await
    }

    async fn confirm_itinerary(
        &self,
        request: ConfirmItineraryRequest,
    ) -> Result<tonic::Response<ConfirmItineraryResponse>, tonic::Status> {
        grpc_info!("(confirm_itinerary) {} client.", self.get_name());
        grpc_debug!("(confirm_itinerary) request: {:?}", request);
        let mut client = self.get_client().await?;
        let mut tries = 0;
        while client.is_ready(ReadyRequest {}).await.is_err() && tries < 5 {
            grpc_info!(
                "(confirm_itinerary) Client not ready yet at attempt {}, retry in 1 sec...",
                tries
            );
            tokio::time::sleep(tokio::time::Duration::new(2, 0)).await;
            tries += 1;
        }
        client.confirm_itinerary(request).await
    }

    async fn cancel_itinerary(
        &self,
        request: Id,
    ) -> Result<tonic::Response<CancelItineraryResponse>, tonic::Status> {
        grpc_info!("(cancel_itinerary) {} client.", self.get_name());
        grpc_debug!("(cancel_itinerary) request: {:?}", request);
        let mut client = self.get_client().await?;
        let mut tries = 0;
        while client.is_ready(ReadyRequest {}).await.is_err() && tries < 5 {
            grpc_info!(
                "(get_client) Client not ready yet at attempt {}, retry in 1 sec...",
                tries
            );
            tokio::time::sleep(tokio::time::Duration::new(1, 0)).await;
            tries += 1;
        }
        client.cancel_itinerary(request).await
    }
}

#[cfg(feature = "stub_client")]
#[async_trait]
impl crate::service::Client<RpcServiceClient<Channel>> for GrpcClient<RpcServiceClient<Channel>> {
    type ReadyRequest = ReadyRequest;
    type ReadyResponse = ReadyResponse;

    async fn is_ready(
        &self,
        request: Self::ReadyRequest,
    ) -> Result<tonic::Response<Self::ReadyResponse>, tonic::Status> {
        grpc_warn!("(is_ready MOCK) {} client.", self.get_name());
        grpc_debug!("(is_ready MOCK) request: {:?}", request);
        Ok(tonic::Response::new(ReadyResponse { ready: true }))
    }

    async fn query_flight(
        &self,
        request: QueryFlightRequest,
    ) -> Result<tonic::Response<QueryFlightResponse>, tonic::Status> {
        grpc_warn!("(query_flight MOCK) {} client.", self.get_name());
        grpc_debug!("(query_flight MOCK) request: {:?}", request);
        let flight_plan_data = svc_storage_client_grpc::flight_plan::mock::get_future_data_obj();
        let flight_plan = svc_storage_client_grpc::flight_plan::Object {
            id: uuid::Uuid::new_v4().to_string(),
            data: Some(flight_plan_data),
        };

        let itineraries = vec![Itinerary {
            id: uuid::Uuid::new_v4().to_string(),
            flight_plan: Some(flight_plan),
            deadhead_flight_plans: vec![],
        }];

        Ok(tonic::Response::new(QueryFlightResponse { itineraries }))
    }

    async fn confirm_itinerary(
        &self,
        request: ConfirmItineraryRequest,
    ) -> Result<tonic::Response<ConfirmItineraryResponse>, tonic::Status> {
        grpc_warn!("(confirm_itinerary MOCK) {} client.", self.get_name());
        grpc_debug!("(confirm_itinerary MOCK) request: {:?}", request);
        Ok(tonic::Response::new(ConfirmItineraryResponse {
            id: uuid::Uuid::new_v4().to_string(),
            confirmed: true,
            confirmation_time: Some(chrono::Utc::now().into()),
        }))
    }

    async fn cancel_itinerary(
        &self,
        request: Id,
    ) -> Result<tonic::Response<CancelItineraryResponse>, tonic::Status> {
        grpc_info!("(cancel_itinerary) {} client.", self.get_name());
        grpc_debug!("(cancel_itinerary) request: {:?}", request);
        Ok(tonic::Response::new(CancelItineraryResponse {
            id: uuid::Uuid::new_v4().to_string(),
            cancelled: true,
            cancellation_time: Some(chrono::Utc::now().into()),
            reason: String::from("Cancelled by user."),
        }))
    }
}

#[cfg(test)]
mod tests {
    use crate::service::Client as ServiceClient;

    use super::*;
    use tonic::transport::Channel;

    #[tokio::test]
    #[cfg(not(feature = "stub_client"))]
    async fn test_client_connect() {
        let name = "scheduler";
        let (server_host, server_port) =
            lib_common::grpc::get_endpoint_from_env("GRPC_HOST", "GRPC_PORT");

        let client: GrpcClient<RpcServiceClient<Channel>> =
            GrpcClient::new_client(&server_host, server_port, name);
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

        let client: GrpcClient<RpcServiceClient<Channel>> =
            GrpcClient::new_client(&server_host, server_port, name);
        assert_eq!(client.get_name(), name);

        let result = client.is_ready(ReadyRequest {}).await;
        println!("{:?}", result);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().into_inner().ready, true);
    }
}
