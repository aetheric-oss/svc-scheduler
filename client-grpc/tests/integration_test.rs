//! Example for writing an integration test.
//! More information: https://doc.rust-lang.org/book/testing-rust.html#integration-tests

use lib_common::grpc::get_endpoint_from_env;
use std::time::SystemTime;
use svc_scheduler_client_grpc::prelude::{scheduler::*, *};
use tokio::sync::OnceCell;

pub(crate) static CLIENT: OnceCell<SchedulerClient> = OnceCell::const_new();

pub async fn get_client() -> &'static SchedulerClient {
    CLIENT
        .get_or_init(|| async move {
            let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
            SchedulerClient::new_client(&host, port, "scheduler")
        })
        .await
}

#[tokio::test]
async fn test_flights_query() -> Result<(), Box<dyn std::error::Error>> {
    let client = get_client().await;
    let sys_time = SystemTime::now();
    let request = QueryFlightRequest {
        is_cargo: true,
        persons: Some(0),
        weight_grams: Some(5000),
        earliest_departure_time: Some(sys_time.into()),
        latest_arrival_time: None,
        vertiport_depart_id: "123".to_string(),
        vertiport_arrive_id: "456".to_string(),
    };

    let response = client.query_flight(request).await?;
    //println!("RESPONSE={:?}", response.into_inner());
    assert_eq!(response.into_inner().itineraries.len(), 1);
    Ok(())
}

#[tokio::test]
async fn test_cancel_itinerary() -> Result<(), Box<dyn std::error::Error>> {
    let client = get_client().await;
    let request = Id {
        id: "1234".to_string(),
    };

    let response = client.cancel_itinerary(request).await?;
    //println!("RESPONSE={:?}", response.into_inner());
    assert_eq!(response.into_inner().cancelled, true);
    Ok(())
}

#[tokio::test]
async fn test_confirm_itinerary() -> Result<(), Box<dyn std::error::Error>> {
    let client = get_client().await;
    let request = ConfirmItineraryRequest {
        id: "1234".to_string(),
        user_id: "".to_string(),
    };

    let response = client.confirm_itinerary(request).await?;
    //println!("RESPONSE={:?}", response.into_inner());
    assert_eq!(response.into_inner().confirmed, true);
    Ok(())
}

#[tokio::test]
async fn test_is_ready() -> Result<(), Box<dyn std::error::Error>> {
    let client = get_client().await;
    let request = ReadyRequest {};

    let response = client.is_ready(request).await?;
    //println!("RESPONSE={:?}", response.into_inner());
    assert_eq!(response.into_inner().ready, true);
    Ok(())
}
