//! gRPC client implementation

///module svc_scheduler generated from svc-scheduler-grpc.proto
pub mod scheduler_grpc {
    #![allow(unused_qualifications)]
    include!("../src/grpc.rs");
}
use chrono::{TimeZone, Utc};
use prost_types::Timestamp;
use scheduler_grpc::scheduler_rpc_client::SchedulerRpcClient;
use scheduler_grpc::{Id, QueryFlightRequest};
use tonic::Request;

/// Example svc-scheduler-client
/// Assuming the server is running on localhost:50051, this method calls `client.query_flight` and
/// should receive a valid response from the server
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = SchedulerRpcClient::connect("http://[::1]:50052").await?;

    let departure_time = Utc
        .with_ymd_and_hms(2022, 10, 25, 15, 0, 0)
        .unwrap()
        .timestamp();

    let request = Request::new(QueryFlightRequest {
        is_cargo: true,
        persons: Some(0),
        weight_grams: Some(5000),
        vertiport_depart_id: "a3509e85-6421-4dd1-8593-74950b88577e".to_string(),
        vertiport_arrive_id: "99c3bd83-79ef-4044-a903-5dac6f557193".to_string(),
        earliest_departure_time: Some(Timestamp {
            seconds: departure_time,
            nanos: 0,
        }),
        latest_arrival_time: None,
    });

    let response = client.query_flight(request).await?.into_inner().flights;
    let id = (&response)[0].flight_plan.as_ref().unwrap().id.to_string();
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
