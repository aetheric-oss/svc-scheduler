//! Example for writing an integration test.
//! More information: https://doc.rust-lang.org/book/testing-rust.html#integration-tests

use chrono::{Duration, Utc};
use lib_common::grpc::get_endpoint_from_env;
use svc_scheduler_client_grpc::prelude::{scheduler::*, *};

#[tokio::test]
async fn test_flights_query() -> Result<(), Box<dyn std::error::Error>> {
    let (server_host, server_port) = get_endpoint_from_env("GRPC_HOST", "GRPC_PORT");
    let client = SchedulerClient::new_client(&server_host, server_port, "scheduler");
    let seconds = Utc::now().timestamp();
    let request = QueryFlightRequest {
        is_cargo: true,
        persons: Some(0),
        weight_grams: Some(5000),
        earliest_departure_time: Some(Timestamp { seconds, nanos: 0 }),
        latest_arrival_time: None,
        origin_vertiport_id: uuid::Uuid::new_v4().to_string(),
        target_vertiport_id: uuid::Uuid::new_v4().to_string(),
        priority: FlightPriority::Low.into(),
    };

    let response = client.query_flight(request).await?;
    //println!("RESPONSE={:?}", response.into_inner());
    assert_eq!(response.into_inner().itineraries.len(), 1);
    Ok(())
}

#[tokio::test]
async fn test_cancel_itinerary() -> Result<(), Box<dyn std::error::Error>> {
    let (server_host, server_port) = get_endpoint_from_env("GRPC_HOST", "GRPC_PORT");
    let client = SchedulerClient::new_client(&server_host, server_port, "scheduler");
    let request = CancelItineraryRequest {
        priority: FlightPriority::Low.into(),
        itinerary_id: uuid::Uuid::new_v4().to_string(),
        user_id: uuid::Uuid::new_v4().to_string(),
    };

    let response = client.cancel_itinerary(request.clone()).await?.into_inner();
    println!("RESPONSE={:?}", response);
    let metadata = response.task_metadata.unwrap();
    assert_eq!(metadata.action, TaskAction::CancelItinerary as i32);

    assert_eq!(metadata.user_id, request.user_id);
    Ok(())
}

#[tokio::test]
async fn test_create_itinerary() -> Result<(), Box<dyn std::error::Error>> {
    let (server_host, server_port) = get_endpoint_from_env("GRPC_HOST", "GRPC_PORT");
    let client = SchedulerClient::new_client(&server_host, server_port, "scheduler");
    let request = CreateItineraryRequest {
        priority: FlightPriority::Low.into(),
        flight_plans: vec![],
        expiry: Some(Timestamp {
            seconds: (Utc::now() + Duration::hours(1)).timestamp(),
            nanos: 0,
        }),
        user_id: uuid::Uuid::new_v4().to_string(),
    };

    let response = client.create_itinerary(request.clone()).await?.into_inner();
    println!("RESPONSE={:?}", response);
    let metadata = response.task_metadata.unwrap();
    assert_eq!(metadata.action, TaskAction::CreateItinerary as i32);

    assert_eq!(metadata.user_id, request.user_id);
    Ok(())
}

#[tokio::test]
async fn test_is_ready() -> Result<(), Box<dyn std::error::Error>> {
    let (server_host, server_port) = get_endpoint_from_env("GRPC_HOST", "GRPC_PORT");
    let client = SchedulerClient::new_client(&server_host, server_port, "scheduler");
    let request = ReadyRequest {};

    let response = client.is_ready(request).await?;
    //println!("RESPONSE={:?}", response.into_inner());
    assert_eq!(response.into_inner().ready, true);
    Ok(())
}
