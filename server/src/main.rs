//! gRPC server implementation

///module svc_scheduler generated from svc-scheduler.proto
pub mod svc_scheduler {
    #![allow(unused_qualifications)]
    include!("svc_scheduler.rs");
}

use dotenv::dotenv;
use std::env;
use std::time::SystemTime;
use svc_scheduler::scheduler_server::{Scheduler, SchedulerServer};
use svc_scheduler::{
    CancelFlightResponse, ConfirmFlightResponse, FlightPriority, FlightStatus, Id, QueryFlightPlan,
    QueryFlightRequest, QueryFlightResponse, ReadyRequest, ReadyResponse,
};
use svc_storage_client::svc_storage::storage_client::StorageClient;
use svc_storage_client::svc_storage::AircraftFilter;
use tonic::{transport::Server, Request, Response, Status};

//static mut STORAGE_CLIENT: StorageClient<tonic::transport::Channel> = None;

///Implementation of gRPC endpoints
#[derive(Debug, Default, Copy, Clone)]
pub struct SchedulerImpl {}

/*impl SchedulerImpl {
    #[tonic::async_trait]
    pub async fn get_aircrafts() -> Result<(), Box<dyn std::error::Error>> {
        let mut client = StorageClient::connect("http://[::1]:50052").await?;
        let sys_time = SystemTime::now();
        let request = tonic::Request::new(AircraftFilter {});

        let response = client.aircrafts(request).await?;

        println!("RESPONSE={:?}", response.into_inner());

        Ok(())
    }
}*/

#[tonic::async_trait]
impl Scheduler for SchedulerImpl {
    ///finds the first possible flight for customer location, flight type and requested time.
    /// Returns draft QueryFlightPlan which can be confirmed or cancelled.
    async fn query_flight(
        &self,
        request: Request<QueryFlightRequest>, // Accept request of type QueryFlightRequest
    ) -> Result<Response<QueryFlightResponse>, Status> {
        // TODO implement. Currently returns arbitrary value
        println!("Got a request: {:?}", request);
        let requested_time = request.into_inner().requested_time;
        let item = QueryFlightPlan {
            id: 1234,
            pilot_id: 1,
            aircraft_id: 1,
            cargo: [123].to_vec(),
            weather_conditions: "Sunny, no wind :)".to_string(),
            vertiport_id_departure: 1,
            pad_id_departure: 1,
            vertiport_id_destination: 1,
            pad_id_destination: 1,
            estimated_departure: requested_time.clone(),
            estimated_arrival: requested_time,
            actual_departure: None,
            actual_arrival: None,
            flight_release_approval: None,
            flight_plan_submitted: None,
            flight_status: FlightStatus::Ready as i32,
            flight_priority: FlightPriority::Low as i32,
        };
        let response = QueryFlightResponse {
            flights: [item].to_vec(),
        };

        Ok(Response::new(response)) // Send back response
    }

    ///Confirms the draft flight plan by id.
    async fn confirm_flight(
        &self,
        _request: Request<Id>,
    ) -> Result<Response<ConfirmFlightResponse>, Status> {
        // TODO implement. Currently returns arbitrary value
        let sys_time = SystemTime::now();
        let response = ConfirmFlightResponse {
            id: 1234,
            confirmed: true,
            confirmation_time: Some(prost_types::Timestamp::from(sys_time)),
        };
        Ok(Response::new(response))
    }

    ///Cancels the draft flight plan by id.
    async fn cancel_flight(
        &self,
        _request: Request<Id>,
    ) -> Result<Response<CancelFlightResponse>, Status> {
        // TODO implement. Currently returns arbitrary value
        let sys_time = SystemTime::now();
        let response = CancelFlightResponse {
            id: 1234,
            cancelled: true,
            cancellation_time: Some(prost_types::Timestamp::from(sys_time)),
            reason: "user cancelled".into(),
        };
        Ok(Response::new(response))
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
    //todo move storage client to some global state and client queries to specific methods
    let mut storage_client = StorageClient::connect("http://[::1]:50052").await?;
    let request = tonic::Request::new(AircraftFilter {});
    let resp = storage_client.aircrafts(request).await?;
    println!("RESPONSE={:?}", resp.into_inner());

    //start server
    Server::builder()
        .add_service(SchedulerServer::new(scheduler))
        .serve(addr)
        .await?;
    println!("gRPC server running at: {}", address);

    Ok(())
}
