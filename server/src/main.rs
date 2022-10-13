//! gRPC server implementation

///module svc_scheduler generated from svc-scheduler.proto
pub mod svc_scheduler {
    #![allow(unused_qualifications)]
    include!("svc_scheduler.rs");
}

use dotenv::dotenv;
use once_cell::sync::OnceCell;
use std::env;
use svc_scheduler::scheduler_server::{Scheduler, SchedulerServer};
use svc_scheduler::{
    CancelFlightResponse, ConfirmFlightResponse, Id, QueryFlightRequest, QueryFlightResponse,
    ReadyRequest, ReadyResponse,
};
use svc_storage_client::svc_storage::storage_client::StorageClient;
use tonic::{transport::Server, Request, Response, Status};

mod queries;

/// GRPC client for storage service -
/// it has to be cloned before each call as per https://github.com/hyperium/tonic/issues/285
pub static STORAGE_CLIENT: OnceCell<StorageClient<tonic::transport::Channel>> = OnceCell::new();

/// shorthand function to clone storage client
pub fn get_storage_client() -> StorageClient<tonic::transport::Channel> {
    STORAGE_CLIENT
        .get()
        .expect("Storage Client not initialized")
        .clone()
}

///Implementation of gRPC endpoints
#[derive(Debug, Default, Copy, Clone)]
pub struct SchedulerImpl {}

#[tonic::async_trait]
impl Scheduler for SchedulerImpl {
    ///finds the first possible flight for customer location, flight type and requested time.
    /// Returns draft QueryFlightPlan which can be confirmed or cancelled.
    async fn query_flight(
        &self,
        request: Request<QueryFlightRequest>,
    ) -> Result<Response<QueryFlightResponse>, Status> {
        queries::query_flight(request, get_storage_client()).await
    }

    ///Confirms the draft flight plan by id.
    async fn confirm_flight(
        &self,
        request: Request<Id>,
    ) -> Result<Response<ConfirmFlightResponse>, Status> {
        queries::confirm_flight(request, get_storage_client()).await
    }

    ///Cancels the draft flight plan by id.
    async fn cancel_flight(
        &self,
        request: Request<Id>,
    ) -> Result<Response<CancelFlightResponse>, Status> {
        queries::cancel_flight(request, get_storage_client()).await
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

///Main entry point: starts gRPC Server on specified address and port
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    //parse socket address from env variable or take default value
    let address = match env::var("GRPC_SOCKET_ADDR") {
        Ok(val) => val,
        Err(_) => "[::1]:50051".to_string(), // default value
    };
    let addr = address.parse()?;
    let scheduler = SchedulerImpl::default();
    //initialize storage client here so it can be used in other methods
    STORAGE_CLIENT
        .set(StorageClient::connect("http://[::1]:50052").await?)
        .unwrap();

    //start server
    Server::builder()
        .add_service(SchedulerServer::new(scheduler))
        .serve(addr)
        .await?;
    println!("gRPC server running at: {}", address);

    Ok(())
}
