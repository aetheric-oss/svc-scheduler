//! gRPC client implementation

///module svc_scheduler generated from svc-scheduler.proto
pub mod svc_scheduler {
    #![allow(unused_qualifications)]
    include!("../src/svc_scheduler.rs");
}
use std::time::SystemTime;
use svc_scheduler::scheduler_client::SchedulerClient;
use svc_scheduler::QueryFlightRequest;

/// Example svc-scheduler-client
/// Assuming the server is running on localhost:50051, this method calls `client.query_flight` and
/// should receive a valid response from the server
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = SchedulerClient::connect("http://[::1]:50051").await?;
    let sys_time = SystemTime::now();
    let request = tonic::Request::new(QueryFlightRequest {
        is_cargo: true,
        persons: 0,
        weight_grams: 5000,
        latitude: 37.77397,
        longitude: -122.43129,
        requested_time: Some(prost_types::Timestamp::from(sys_time)),
    });

    let response = client.query_flight(request).await?;

    println!("RESPONSE={:?}", response.into_inner());

    Ok(())
}
