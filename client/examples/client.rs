//! gRPC client implementation

///module svc_scheduler generated from svc-scheduler-grpc.proto
pub mod scheduler_grpc {
    #![allow(unused_qualifications)]
    include!("../src/grpc.rs");
}
use chrono::{DateTime, Duration, TimeZone, Utc};
use chrono_tz::Tz;
use prost_types::Timestamp;
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

    let departure_time = Utc.ymd(2022, 10, 25).and_hms(15, 0, 0).timestamp();

    let request = Request::new(QueryFlightRequest {
        is_cargo: true,
        persons: Some(0),
        weight_grams: Some(5000),
        vertiport_depart_id: "a6aa2f85-afd6-4079-abc1-84e41a05b449".to_string(),
        vertiport_arrive_id: "9d14eb4f-4ef6-4ba5-84dd-6536f98b8516".to_string(),
        departure_time: Some(Timestamp {
            seconds: departure_time,
            nanos: 0,
        }),
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
