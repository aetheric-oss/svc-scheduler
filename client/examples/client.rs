//! gRPC client implementation

///module svc_scheduler generated from svc-scheduler-grpc.proto
pub mod scheduler_grpc {
    #![allow(unused_qualifications)]
    include!("../src/grpc.rs");
}
use scheduler_grpc::scheduler_rpc_client::SchedulerRpcClient;
use scheduler_grpc::{Id, QueryFlightRequest};
use std::time::SystemTime;
use tonic::Request;

/// Example svc-scheduler-client
/// Assuming the server is running on localhost:50051, this method calls `client.query_flight` and
/// should receive a valid response from the server
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = SchedulerRpcClient::connect("http://[::1]:50051").await?;
    let sys_time = SystemTime::now();
    let request = Request::new(QueryFlightRequest {
        is_cargo: true,
        persons: Some(0),
        weight_grams: Some(5000),
        vertiport_depart_id: "123".to_string(),
        vertiport_arrive_id: "456".to_string(),
        departure_time: Some(prost_types::Timestamp::from(sys_time)),
        arrival_time: None,
    });

    let response = client.query_flight(request).await?.into_inner().flights;
    let id = (&response)[0].id.to_string();
    println!("id={}", id);
    /*let response = client
    .cancel_flight(Request::new(Id {
        id: "b32c8a28-bfb4-4fe9-8819-e119e18991c0".to_string(),
    }))
    .await?;*/
    let response = client.confirm_flight(Request::new(Id { id })).await?;

    println!("RESPONSE={:?}", &response);

    Ok(())
}
