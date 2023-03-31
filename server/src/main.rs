//! gRPC server implementation

///module svc_scheduler generated from svc-scheduler-grpc.proto
pub mod scheduler_grpc {
    #![allow(unused_qualifications)]
    include!("grpc.rs");
}
mod grpc_client_wrapper;
///Queries module
pub mod queries;

pub mod router;

use std::thread::sleep;

use router::router_utils::router_state::{init_router_from_vertiports, is_router_initialized};

use dotenv::dotenv;
use tokio::sync::OnceCell;

#[macro_use]
extern crate log;

use crate::grpc_client_wrapper::{
    ComplianceClientWrapper, GRPCClients, StorageClientWrapper, StorageClientWrapperTrait,
};
use scheduler_grpc::scheduler_rpc_server::{SchedulerRpc, SchedulerRpcServer};
use scheduler_grpc::{
    CancelItineraryResponse, ConfirmItineraryRequest, ConfirmItineraryResponse, Id,
    QueryFlightRequest, QueryFlightResponse, ReadyRequest, ReadyResponse,
};
use svc_compliance_client_grpc::client::compliance_rpc_client::ComplianceRpcClient;
use svc_storage_client_grpc::AdvancedSearchFilter;
use svc_storage_client_grpc::{
    FlightPlanClient, ItineraryClient, ItineraryFlightPlanLinkClient, VehicleClient,
    VertipadClient, VertiportClient,
};
use tonic::{transport::Server, Request, Response, Status};

/// GRPC clients for storage service
/// They have to be cloned before each call as per <https://github.com/hyperium/tonic/issues/285>

pub(crate) static STORAGE_CLIENT_WRAPPER: OnceCell<StorageClientWrapper> = OnceCell::const_new();
pub(crate) static COMPLIANCE_CLIENT_WRAPPER: OnceCell<ComplianceClientWrapper> =
    OnceCell::const_new();

pub(crate) fn get_storage_client_wrapper() -> &'static StorageClientWrapper {
    STORAGE_CLIENT_WRAPPER
        .get()
        .expect("Storage clients not initialized")
}

pub(crate) fn get_compliance_client_wrapper() -> &'static ComplianceClientWrapper {
    COMPLIANCE_CLIENT_WRAPPER
        .get()
        .expect("Compliance client not initialized")
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
            error!("{}", res.as_ref().err().unwrap());
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
    let result = get_storage_client_wrapper()
        .vertiports(Request::new(AdvancedSearchFilter {
            filters: vec![],
            page_number: 0,
            results_per_page: 50,
            order_by: vec![],
        }))
        .await;

    let Ok(vertiports) = result else {
        let error_msg = "Failed to get vertiports from storage service".to_string();
        debug!("{}: {:?}", error_msg, result.unwrap_err());
        panic!("{}", error_msg);
    };

    let vertiports = vertiports.into_inner().list;
    info!("Initializing router with {} vertiports ", vertiports.len());
    if !is_router_initialized() {
        let res = init_router_from_vertiports(&vertiports);
        if res.is_err() {
            error!("Failed to initialize router: {}", res.err().unwrap());
        }
    }
}

/// The GRPC Server for this service
async fn grpc_server() {
    // GRPC Server
    let grpc_port = std::env::var("DOCKER_PORT_GRPC")
        .unwrap_or_else(|_| "50051".to_string())
        .parse::<u16>()
        .unwrap_or(50051);
    let full_grpc_addr = format!("[::]:{grpc_port}").parse().unwrap();

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
        .await
        .unwrap();

    info!("gRPC Server Listening at {}", full_grpc_addr);
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

    // Compliance GRPC Server
    let compliance_grpc_port = std::env::var("COMPLIANCE_PORT_GRPC")
        .unwrap_or_else(|_| "50051".to_string())
        .parse::<u16>()
        .unwrap_or(50051);
    let compliance_grpc_host =
        std::env::var("COMPLIANCE_HOST_GRPC").unwrap_or_else(|_| "localhost".to_string());

    let compliance_full_grpc_addr =
        format!("http://{compliance_grpc_host}:{compliance_grpc_port}").to_string();

    info!(
        "Setting up connection to svc-storage clients on {}",
        storage_full_grpc_addr.clone()
    );

    let flight_plan_client_res = FlightPlanClient::connect(storage_full_grpc_addr.clone()).await;
    let vehicle_client_res = VehicleClient::connect(storage_full_grpc_addr.clone()).await;
    let vertiport_client_res = VertiportClient::connect(storage_full_grpc_addr.clone()).await;
    let vertipad_client_res = VertipadClient::connect(storage_full_grpc_addr.clone()).await;
    let itinerary_client_res = ItineraryClient::connect(storage_full_grpc_addr.clone()).await;
    let itinerary_fp_client_res =
        ItineraryFlightPlanLinkClient::connect(storage_full_grpc_addr.clone()).await;

    let compliance_client_res =
        ComplianceRpcClient::connect(compliance_full_grpc_addr.clone()).await;
    if flight_plan_client_res.is_err()
        || vehicle_client_res.is_err()
        || vertiport_client_res.is_err()
        || vertipad_client_res.is_err()
        || itinerary_client_res.is_err()
        || itinerary_fp_client_res.is_err()
    {
        error!(
            "Failed to connect to storage service at {}. Client errors: {} {} {} {} {} {}",
            storage_full_grpc_addr.clone(),
            flight_plan_client_res.err().unwrap(),
            vehicle_client_res.err().unwrap(),
            vertiport_client_res.err().unwrap(),
            vertipad_client_res.err().unwrap(),
            itinerary_client_res.err().unwrap(),
            itinerary_fp_client_res.err().unwrap()
        );
        panic!();
    } else if compliance_client_res.is_err() {
        error!(
            "Failed to connect to compliance service at {}. Client errors: {}",
            storage_full_grpc_addr.clone(),
            compliance_client_res.err().unwrap()
        );
        panic!();
    } else {
        let grpc_clients = GRPCClients {
            flight_plan_client: flight_plan_client_res.unwrap(),
            vehicle_client: vehicle_client_res.unwrap(),
            vertiport_client: vertiport_client_res.unwrap(),
            vertipad_client: vertipad_client_res.unwrap(),
            compliance_client: compliance_client_res.unwrap(),
            itinerary_client: itinerary_client_res.unwrap(),
            itinerary_fp_link_client: itinerary_fp_client_res.unwrap(),
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

    // Spawn the loop for the router re-initialization
    tokio::spawn(async move {
        let duration = std::time::Duration::new(10, 0);
        loop {
            init_router().await;

            // TODO R3: On trigger from svc-assets or svc-storage
            sleep(duration);
        }
    });

    // Spawn the GRPC server for this service
    let _ = tokio::spawn(grpc_server()).await;

    Ok(())
}
