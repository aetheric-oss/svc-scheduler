//! gRPC server implementation

///module svc_scheduler generated from svc-scheduler-grpc.proto
pub mod scheduler_grpc {
    #![allow(unused_qualifications)]
    include!("grpc.rs");
}
///Queries module
pub mod queries;
use router::router_state::{init_router_from_vertiports, is_router_initialized};

use dotenv::dotenv;
use once_cell::sync::OnceCell;

use scheduler_grpc::scheduler_rpc_server::{SchedulerRpc, SchedulerRpcServer};
use scheduler_grpc::{
    CancelFlightResponse, ConfirmFlightResponse, Id, QueryFlightRequest, QueryFlightResponse,
    ReadyRequest, ReadyResponse,
};
use svc_storage_client_grpc::client::{
    flight_plan_rpc_client::FlightPlanRpcClient, vehicle_rpc_client::VehicleRpcClient,
    vertiport_rpc_client::VertiportRpcClient, SearchFilter,
};

use tonic::{transport::Channel, transport::Server, Request, Response, Status};

/// GRPC clients for storage service -
/// it has to be cloned before each call as per https://github.com/hyperium/tonic/issues/285
pub static VEHICLE_CLIENT: OnceCell<VehicleRpcClient<Channel>> = OnceCell::new();
/// Vertiport client
pub static VERTIPORT_CLIENT: OnceCell<VertiportRpcClient<Channel>> = OnceCell::new();
/// Flight Plan client
pub static FLIGHT_PLAN_CLIENT: OnceCell<FlightPlanRpcClient<Channel>> = OnceCell::new();

/// shorthand function to clone vehicle client
pub fn get_vehicle_client() -> VehicleRpcClient<Channel> {
    VEHICLE_CLIENT
        .get()
        .expect("Storage Client not initialized")
        .clone()
}

/// shorthand function to clone vertiport client
pub fn get_vertiport_client() -> VertiportRpcClient<Channel> {
    VERTIPORT_CLIENT
        .get()
        .expect("Storage Client not initialized")
        .clone()
}

/// shorthand function to clone flight plan client
pub fn get_flight_plan_client() -> FlightPlanRpcClient<Channel> {
    FLIGHT_PLAN_CLIENT
        .get()
        .expect("Storage Client not initialized")
        .clone()
}

///Implementation of gRPC endpoints
#[derive(Debug, Default, Copy, Clone)]
pub struct SchedulerGrpcImpl {}

#[tonic::async_trait]
impl SchedulerRpc for SchedulerGrpcImpl {
    ///finds the first possible flight for customer location, flight type and requested time.
    /// Returns draft QueryFlightPlan which can be confirmed or cancelled.
    async fn query_flight(
        &self,
        request: Request<QueryFlightRequest>,
    ) -> Result<Response<QueryFlightResponse>, Status> {
        queries::query_flight(
            request,
            get_flight_plan_client(),
            get_vehicle_client(),
            get_vertiport_client(),
        )
        .await
    }

    ///Confirms the draft flight plan by id.
    async fn confirm_flight(
        &self,
        request: Request<Id>,
    ) -> Result<Response<ConfirmFlightResponse>, Status> {
        queries::confirm_flight(request, get_flight_plan_client()).await
    }

    ///Cancels the draft flight plan by id.
    async fn cancel_flight(
        &self,
        request: Request<Id>,
    ) -> Result<Response<CancelFlightResponse>, Status> {
        queries::cancel_flight(request).await
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

async fn init_router(mut vertiport_client: VertiportRpcClient<Channel>) {
    let vertiports = vertiport_client
        .vertiports(Request::new(SearchFilter {
            search_field: "".to_string(),
            search_value: "".to_string(),
            page_number: 0,
            results_per_page: 50,
        }))
        .await
        .unwrap()
        .into_inner()
        .vertiports;
    println!("Vertiports found: {}", vertiports.len());
    if !is_router_initialized() {
        init_router_from_vertiports(&vertiports);
    }
}

///Main entry point: starts gRPC Server on specified address and port
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    //initialize storage client here so it can be used in other methods
    //todo change url to env variable
    FLIGHT_PLAN_CLIENT
        .set(FlightPlanRpcClient::connect("http://[::1]:50052").await?)
        .unwrap();
    VERTIPORT_CLIENT
        .set(VertiportRpcClient::connect("http://[::1]:50052").await?)
        .unwrap();
    VEHICLE_CLIENT
        .set(VehicleRpcClient::connect("http://[::1]:50052").await?)
        .unwrap();
    // Initialize Router from vertiport data
    init_router(get_vertiport_client()).await;
    // GRPC Server
    let grpc_port = std::env::var("DOCKER_PORT_GRPC")
        .unwrap_or_else(|_| "50051".to_string())
        .parse::<u16>()
        .unwrap_or(50051);
    let addr = format!("[::]:{grpc_port}").parse().unwrap();
    println!("gRPC server running at: {}", addr);
    let scheduler = SchedulerGrpcImpl::default();
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<SchedulerRpcServer<SchedulerGrpcImpl>>()
        .await;

    Server::builder()
        .add_service(health_service)
        .add_service(SchedulerRpcServer::new(scheduler))
        .serve(addr)
        .await?;
    println!("gRPC Server Listening at {}", addr);
    Ok(())
}
