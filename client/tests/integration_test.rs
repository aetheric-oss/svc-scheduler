//! Example for writing an integration test.
//! More information: https://doc.rust-lang.org/book/testing-rust.html#integration-tests

use std::time::SystemTime;
use svc_scheduler_client;
use svc_scheduler_client::svc_scheduler::scheduler_client::SchedulerClient;
use svc_scheduler_client::svc_scheduler::{Id, QueryFlightRequest, ReadyRequest};

#[tokio::test]
#[ignore = "integration test env not yet implemented"]
async fn test_flights_query() -> Result<(), Box<dyn std::error::Error>> {
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
    //println!("RESPONSE={:?}", response.into_inner());
    assert_eq!(response.into_inner().flights.len(), 1);
    Ok(())
}

#[tokio::test]
#[ignore = "integration test env not yet implemented"]
async fn test_cancel_flight() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = SchedulerClient::connect("http://[::1]:50051").await?;
    let request = tonic::Request::new(Id { id: 1234 });

    let response = client.cancel_flight(request).await?;
    //println!("RESPONSE={:?}", response.into_inner());
    assert_eq!(response.into_inner().cancelled, true);
    Ok(())
}

#[tokio::test]
#[ignore = "integration test env not yet implemented"]
async fn test_confirm_flight() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = SchedulerClient::connect("http://[::1]:50051").await?;
    let request = tonic::Request::new(Id { id: 1234 });

    let response = client.confirm_flight(request).await?;
    //println!("RESPONSE={:?}", response.into_inner());
    assert_eq!(response.into_inner().confirmed, true);
    Ok(())
}

#[tokio::test]
#[ignore = "integration test env not yet implemented"]
async fn test_is_ready() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = SchedulerClient::connect("http://[::1]:50051").await?;
    let request = tonic::Request::new(ReadyRequest {});

    let response = client.is_ready(request).await?;
    //println!("RESPONSE={:?}", response.into_inner());
    assert_eq!(response.into_inner().ready, true);
    Ok(())
}
