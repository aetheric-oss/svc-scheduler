//! gRPC server implementation

pub mod grpc_server {
    #![allow(unused_qualifications, missing_docs)]
    tonic::include_proto!("grpc");
}
pub use grpc_server::rpc_service_server::{RpcService, RpcServiceServer};
pub use grpc_server::{
    CancelItineraryResponse, ConfirmItineraryRequest, ConfirmItineraryResponse, Id, Itinerary,
    QueryFlightRequest, QueryFlightResponse, ReadyRequest, ReadyResponse,
};

use crate::shutdown_signal;
use crate::Config;

use std::fmt::Debug;
use std::net::SocketAddr;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

/// struct to implement the gRPC server functions
#[derive(Debug, Default, Copy, Clone)]
pub struct ServerImpl {}

#[cfg(not(feature = "stub_server"))]
#[tonic::async_trait]
impl RpcService for ServerImpl {
    ///finds the first possible flight for customer location, flight type and requested time.
    /// Returns draft QueryFlightPlan which can be confirmed or cancelled.
    async fn query_flight(
        &self,
        request: Request<QueryFlightRequest>,
    ) -> Result<Response<QueryFlightResponse>, Status> {
        grpc_info!("(query_flight) scheduler server.");
        grpc_debug!("(query_flight) request: {:?}", request);
        let res = super::queries::query_flight(request).await;
        if res.is_err() {
            grpc_error!("{}", res.as_ref().err().unwrap());
        }
        res
    }

    ///Confirms the draft itinerary by id.
    async fn confirm_itinerary(
        &self,
        request: Request<ConfirmItineraryRequest>,
    ) -> Result<Response<ConfirmItineraryResponse>, Status> {
        grpc_info!("(confirm_itinerary) scheduler server.");
        grpc_debug!("(confirm_itinerary) request: {:?}", request);
        let res = super::queries::confirm_itinerary(request).await;
        if res.is_err() {
            grpc_error!("{}", res.as_ref().err().unwrap());
        }
        res
    }

    /// Cancels the itinerary by id.
    async fn cancel_itinerary(
        &self,
        request: Request<Id>,
    ) -> Result<Response<CancelItineraryResponse>, Status> {
        grpc_info!("(cancel_itinerary) scheduler server.");
        grpc_debug!("(cancel_itinerary) request: {:?}", request);
        let res = super::queries::cancel_itinerary(request).await;
        if res.is_err() {
            grpc_error!("{}", res.as_ref().err().unwrap());
        }
        res
    }

    /// Returns ready:true when service is available
    async fn is_ready(
        &self,
        request: Request<ReadyRequest>,
    ) -> Result<Response<ReadyResponse>, Status> {
        grpc_info!("(is_ready) scheduler server.");
        grpc_debug!("(is_ready) request: {:?}", request);
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }
}

/// Starts the grpc servers for this microservice using the provided configuration
///
/// # Example:
/// ```
/// use svc_scheduler::grpc::server::grpc_server;
/// use svc_scheduler::Config;
/// async fn example() -> Result<(), tokio::task::JoinError> {
///     let config = Config::default();
///     tokio::spawn(grpc_server(config, None)).await;
///     Ok(())
/// }
/// ```
pub async fn grpc_server(config: Config, shutdown_rx: Option<tokio::sync::oneshot::Receiver<()>>) {
    grpc_debug!("(grpc_server) entry.");

    // Grpc Server
    let grpc_port = config.docker_port_grpc;
    let full_grpc_addr: SocketAddr = match format!("[::]:{}", grpc_port).parse() {
        Ok(addr) => addr,
        Err(e) => {
            grpc_error!("(grpc_server) Failed to parse gRPC address: {}", e);
            return;
        }
    };

    let imp = ServerImpl::default();
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<RpcServiceServer<ServerImpl>>()
        .await;

    //start server
    grpc_info!(
        "(grpc_server) Starting gRPC services on: {}.",
        full_grpc_addr
    );
    match Server::builder()
        .add_service(health_service)
        .add_service(RpcServiceServer::new(imp))
        .serve_with_shutdown(full_grpc_addr, shutdown_signal("grpc", shutdown_rx))
        .await
    {
        Ok(_) => grpc_info!("(grpc_server) gRPC server running at: {}.", full_grpc_addr),
        Err(e) => {
            grpc_error!("(grpc_server) could not start gRPC server: {}", e);
        }
    };
}

#[cfg(feature = "stub_server")]
#[tonic::async_trait]
impl RpcService for ServerImpl {
    ///finds the first possible flight for customer location, flight type and requested time.
    /// Returns draft QueryFlightPlan which can be confirmed or cancelled.
    async fn query_flight(
        &self,
        request: Request<QueryFlightRequest>,
    ) -> Result<Response<QueryFlightResponse>, Status> {
        grpc_warn!("(query_flight MOCK) scheduler server.");
        grpc_debug!("(query_flight MOCK) request: {:?}", request);
        let flight_plan_data =
            svc_storage_client_grpc::prelude::flight_plan::mock::get_future_data_obj();
        let flight_plan = svc_storage_client_grpc::prelude::flight_plan::Object {
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

    ///Confirms the draft itinerary by id.
    async fn confirm_itinerary(
        &self,
        request: Request<ConfirmItineraryRequest>,
    ) -> Result<Response<ConfirmItineraryResponse>, Status> {
        grpc_warn!("(confirm_itinerary MOCK) scheduler server.");
        grpc_debug!("(confirm_itinerary MOCK) request: {:?}", request);
        Ok(tonic::Response::new(ConfirmItineraryResponse {
            id: uuid::Uuid::new_v4().to_string(),
            confirmed: true,
            confirmation_time: Some(chrono::Utc::now().into()),
        }))
    }

    /// Cancels the itinerary by id.
    async fn cancel_itinerary(
        &self,
        request: Request<Id>,
    ) -> Result<Response<CancelItineraryResponse>, Status> {
        grpc_warn!("(cancel_itinerary MOCK) scheduler server.");
        grpc_debug!("(cancel_itinerary MOCK) request: {:?}", request);
        Ok(tonic::Response::new(CancelItineraryResponse {
            id: uuid::Uuid::new_v4().to_string(),
            cancelled: true,
            cancellation_time: Some(chrono::Utc::now().into()),
            reason: String::from("Cancelled by user."),
        }))
    }

    /// Returns ready:true when service is available
    async fn is_ready(
        &self,
        request: Request<ReadyRequest>,
    ) -> Result<Response<ReadyResponse>, Status> {
        grpc_warn!("(is_ready MOCK) scheduler server.");
        grpc_debug!("(is_ready MOCK) request: {:?}", request);
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }
}
