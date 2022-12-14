//! gRPC server implementation

///module svc_scheduler generated from svc-scheduler-grpc.proto
pub mod scheduler_grpc {
    #![allow(unused_qualifications)]
    include!("grpc.rs");
}
mod grpc_client_wrapper;
///Queries module
pub mod queries;

use router::router_state::{init_router_from_vertiports, is_router_initialized};

use dotenv::dotenv;
use tokio::sync::OnceCell;

#[macro_use]
extern crate log;

use scheduler_grpc::scheduler_rpc_server::{SchedulerRpc, SchedulerRpcServer};
use scheduler_grpc::{
    CancelFlightResponse, ConfirmFlightResponse, Id, QueryFlightRequest, QueryFlightResponse,
    ReadyRequest, ReadyResponse,
};
use svc_storage_client_grpc::client::vertipad_rpc_client::VertipadRpcClient;
use svc_storage_client_grpc::client::{
    flight_plan_rpc_client::FlightPlanRpcClient, vehicle_rpc_client::VehicleRpcClient,
    vertiport_rpc_client::VertiportRpcClient, SearchFilter,
};

use crate::grpc_client_wrapper::{GRPCClients, StorageClientWrapper, StorageClientWrapperTrait};
use tonic::{transport::Server, Request, Response, Status};

/// GRPC clients for storage service
/// They have to be cloned before each call as per https://github.com/hyperium/tonic/issues/285

pub(crate) static STORAGE_CLIENT_WRAPPER: OnceCell<StorageClientWrapper> = OnceCell::const_new();

pub(crate) fn get_storage_client_wrapper() -> &'static StorageClientWrapper {
    STORAGE_CLIENT_WRAPPER
        .get()
        .expect("Storage clients not initialized")
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
        let res = queries::query_flight(request, get_storage_client_wrapper()).await;
        if res.is_err() {
            error!("{}", res.as_ref().err().unwrap());
        }
        res
    }

    ///Confirms the draft flight plan by id.
    async fn confirm_flight(
        &self,
        request: Request<Id>,
    ) -> Result<Response<ConfirmFlightResponse>, Status> {
        let res = queries::confirm_flight(request, get_storage_client_wrapper()).await;
        if res.is_err() {
            error!("{}", res.as_ref().err().unwrap());
        }
        res
    }

    ///Cancels the draft flight plan by id.
    async fn cancel_flight(
        &self,
        request: Request<Id>,
    ) -> Result<Response<CancelFlightResponse>, Status> {
        let res = queries::cancel_flight(request, get_storage_client_wrapper()).await;
        if res.is_err() {
            error!("{}", res.as_ref().err().unwrap());
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

/// Initializes router state from vertiports from storage service
async fn init_router() {
    let vertiports_res = get_storage_client_wrapper()
        .vertiports(Request::new(SearchFilter {
            search_field: "".to_string(),
            search_value: "".to_string(),
            page_number: 0,
            results_per_page: 50,
        }))
        .await;
    if vertiports_res.is_err() {
        error!("Failed to get vertiports from storage service");
        panic!("Failed to get vertiports from storage service");
    }
    let vertiports = vertiports_res.unwrap().into_inner().vertiports;
    info!("Initializing router with {} vertiports ", vertiports.len());
    if !is_router_initialized() {
        let res = init_router_from_vertiports(&vertiports);
        if res.is_err() {
            error!("Failed to initialize router: {}", res.err().unwrap());
        }
    }
}

/// Initializes grpc clients for storage service
async fn init_grpc_clients() {
    //initialize storage client here so it can be used in other methods
    // Storage GRPC Server
    let storage_grpc_port = std::env::var("STORAGE_PORT_GRPC")
        .unwrap_or_else(|_| "50051".to_string())
        .parse::<u16>()
        .unwrap_or(50051);
    let storage_grpc_host =
        std::env::var("STORAGE_HOST_GRPC").unwrap_or_else(|_| "localhost".to_string());

    let storage_full_grpc_addr =
        format!("http://{storage_grpc_host}:{storage_grpc_port}").to_string();

    info!(
        "Setting up connection to svc-storage clients on {}",
        storage_full_grpc_addr.clone()
    );
    let flight_plan_client_res = FlightPlanRpcClient::connect(storage_full_grpc_addr.clone()).await;
    let vehicle_client_res = VehicleRpcClient::connect(storage_full_grpc_addr.clone()).await;
    let vertiport_client_res = VertiportRpcClient::connect(storage_full_grpc_addr.clone()).await;
    let vertipad_client_res = VertipadRpcClient::connect(storage_full_grpc_addr.clone()).await;
    if flight_plan_client_res.is_err()
        || vehicle_client_res.is_err()
        || vertiport_client_res.is_err()
        || vertipad_client_res.is_err()
    {
        error!(
            "Failed to connect to storage service at {}. Client errors: {} {} {} {}",
            storage_full_grpc_addr.clone(),
            flight_plan_client_res.err().unwrap(),
            vehicle_client_res.err().unwrap(),
            vertiport_client_res.err().unwrap(),
            vertipad_client_res.err().unwrap()
        );
        panic!();
    } else {
        let grpc_clients = GRPCClients {
            flight_plan_client: flight_plan_client_res.unwrap(),
            vehicle_client: vehicle_client_res.unwrap(),
            vertiport_client: vertiport_client_res.unwrap(),
            vertipad_client: vertipad_client_res.unwrap(),
        };
        STORAGE_CLIENT_WRAPPER
            .set(StorageClientWrapper {
                grpc_clients: Some(grpc_clients),
            })
            .expect("Failed to set storage client wrapper");
    }
}

///Main entry point: starts gRPC Server on specified address and port
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //initialize dotenv library which reads .env file
    dotenv().ok();
    //initialize logger
    let log_cfg: &str = "log4rs.yaml";
    if let Err(e) = log4rs::init_file(log_cfg, Default::default()) {
        println!("(logger) could not parse {}. {}", log_cfg, e);
        panic!();
    }
    //initialize storage client here so it can be used in other methods
    init_grpc_clients().await;
    // Initialize Router from vertiport data
    init_router().await;

    // GRPC Server
    let grpc_port = std::env::var("DOCKER_PORT_GRPC")
        .unwrap_or_else(|_| "50051".to_string())
        .parse::<u16>()
        .unwrap_or(50051);
    let full_grpc_addr = format!("[::]:{grpc_port}").parse()?;

    let scheduler = SchedulerGrpcImpl::default();
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<SchedulerRpcServer<SchedulerGrpcImpl>>()
        .await;

    //start server
    info!("Starting gRPC server at: {}", full_grpc_addr);
    Server::builder()
        .add_service(health_service)
        .add_service(SchedulerRpcServer::new(scheduler))
        .serve(full_grpc_addr)
        .await?;
    info!("gRPC Server Listening at {}", full_grpc_addr);

    Ok(())
}
