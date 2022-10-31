//! Example for writing an integration test.
//! More information: https://doc.rust-lang.org/book/testing-rust.html#integration-tests

use std::time::SystemTime;
use svc_scheduler_client::grpc::{
    scheduler_rpc_client::SchedulerRpcClient, Id, QueryFlightRequest, ReadyRequest,
};

#[tokio::test]
#[ignore = "integration test env not yet implemented"]
async fn test_flights_query() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = SchedulerRpcClient::connect("http://[::1]:50051").await?;
    let sys_time = SystemTime::now();
    let request = tonic::Request::new(QueryFlightRequest {
        is_cargo: true,
        persons: Some(0),
        weight_grams: Some(5000),
        departure_time: Some(prost_types::Timestamp::from(sys_time)),
        arrival_time: None,
        vertiport_depart_id: "123".to_string(),
        vertiport_arrive_id: "456".to_string(),
    });

    let response = client.query_flight(request).await?;
    //println!("RESPONSE={:?}", response.into_inner());
    assert_eq!(response.into_inner().flights.len(), 1);
    Ok(())
}

#[tokio::test]
#[ignore = "integration test env not yet implemented"]
async fn test_cancel_flight() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = SchedulerRpcClient::connect("http://[::1]:50051").await?;
    let request = tonic::Request::new(Id {
        id: "1234".to_string(),
    });

    let response = client.cancel_flight(request).await?;
    //println!("RESPONSE={:?}", response.into_inner());
    assert_eq!(response.into_inner().cancelled, true);
    Ok(())
}

#[tokio::test]
#[ignore = "integration test env not yet implemented"]
async fn test_confirm_flight() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = SchedulerRpcClient::connect("http://[::1]:50051").await?;
    let request = tonic::Request::new(Id {
        id: "1234".to_string(),
    });

    let response = client.confirm_flight(request).await?;
    //println!("RESPONSE={:?}", response.into_inner());
    assert_eq!(response.into_inner().confirmed, true);
    Ok(())
}

#[tokio::test]
#[ignore = "integration test env not yet implemented"]
async fn test_is_ready() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = SchedulerRpcClient::connect("http://[::1]:50051").await?;
    let request = tonic::Request::new(ReadyRequest {});

    let response = client.is_ready(request).await?;
    //println!("RESPONSE={:?}", response.into_inner());
    assert_eq!(response.into_inner().ready, true);
    Ok(())
}
