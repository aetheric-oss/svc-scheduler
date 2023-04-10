pub mod grpc_server {
    #![allow(unused_qualifications, missing_docs)]
    tonic::include_proto!("grpc");
}

use super::client::{get_compliance_client_wrapper, get_storage_client_wrapper};

use crate::queries;

use grpc_server::{
    CancelItineraryResponse, ConfirmItineraryRequest, ConfirmItineraryResponse, Id,
    QueryFlightRequest, QueryFlightResponse, ReadyRequest, ReadyResponse,
};

use grpc_server::rpc_service_server::{RpcService, RpcServiceServer};

use tonic::transport::Server;
use tonic::{Request, Response, Status};

///Implementation of gRPC endpoints
#[derive(Debug, Default, Copy, Clone)]
pub struct GrpcServerImpl {}

#[tonic::async_trait]
impl RpcService for GrpcServerImpl {
    ///finds the first possible flight for customer location, flight type and requested time.
    /// Returns draft QueryFlightPlan which can be confirmed or cancelled.
    async fn query_flight(
        &self,
        request: Request<QueryFlightRequest>,
    ) -> Result<Response<QueryFlightResponse>, Status> {
        let res = queries::query_flight(request, get_storage_client_wrapper()).await;
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
        let res = queries::confirm_itinerary(
            request,
            get_storage_client_wrapper(),
            get_compliance_client_wrapper(),
        )
        .await;
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
        let res = queries::cancel_itinerary(request, get_storage_client_wrapper()).await;
        if res.is_err() {
            grpc_error!("{}", res.as_ref().err().unwrap());
        }
        res
    }

    /// Returns ready:true when service is available
    async fn is_ready(
        &self,
        _request: Request<ReadyRequest>,
    ) -> Result<Response<ReadyResponse>, Status> {
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }
}

/// The GRPC Server for this service
pub async fn server(config: crate::config::Config) {
    // GRPC Server
    let grpc_port = config.docker_port_grpc;
    let full_grpc_addr = format!("[::]:{grpc_port}").parse().unwrap();

    let scheduler = GrpcServerImpl::default();
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<RpcServiceServer<GrpcServerImpl>>()
        .await;

    //start server
    grpc_info!("(grpc_server) starting gRPC server at {}.", full_grpc_addr);
    Server::builder()
        .add_service(health_service)
        .add_service(RpcServiceServer::new(scheduler))
        .serve(full_grpc_addr)
        .await
        .unwrap();
}
