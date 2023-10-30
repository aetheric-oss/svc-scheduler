//! gRPC client implementation

use chrono::Utc;
use lib_common::grpc::{get_endpoint_from_env, GrpcClient};
use svc_scheduler_client_grpc::{
    client::rpc_service_client::RpcServiceClient,
    prelude::{scheduler::*, *},
};
use tonic::transport::Channel;

/// Example querying a flight from svc-scheduler
async fn query_itinerary_example(
    client: &GrpcClient<RpcServiceClient<Channel>>,
) -> Option<Vec<Itinerary>> {
    let itinerary_date = (Utc::now() + chrono::Duration::days(10)).date_naive();
    let departure_time = itinerary_date.and_hms_opt(8, 0, 0).unwrap().and_utc();
    let arrival_time = itinerary_date.and_hms_opt(9, 0, 0).unwrap().and_utc();
    let origin_vertiport_id = uuid::Uuid::new_v4().to_string();
    let target_vertiport_id = uuid::Uuid::new_v4().to_string();

    let request = QueryFlightRequest {
        is_cargo: true,
        persons: None,
        weight_grams: Some(5000),
        origin_vertiport_id,
        target_vertiport_id,
        earliest_departure_time: Some(departure_time.into()),
        latest_arrival_time: Some(arrival_time.into()),
        priority: FlightPriority::Low as i32,
    };

    match client.query_flight(request).await {
        Ok(response) => {
            let itineraries = response.into_inner().itineraries;
            println!("(main) RESPONSE={:?}", &itineraries);
            Some(itineraries)
        }
        Err(e) => {
            println!("(main) ERROR={:?}", e);
            None
        }
    }
}

/// Example creating a flight from svc-scheduler
async fn create_itinerary_example(
    client: &GrpcClient<RpcServiceClient<Channel>>,
    itinerary: &Itinerary,
) -> Option<i64> {
    let request = CreateItineraryRequest {
        priority: FlightPriority::Low.into(),
        flight_plans: itinerary.flight_plans.clone(),
    };

    match client.create_itinerary(request).await {
        Ok(response) => {
            let task_id = response.into_inner().task_id;
            println!("(main) RESPONSE={:?}", &task_id);
            Some(task_id)
        }
        Err(e) => {
            println!("(main) ERROR={:?}", e);
            None
        }
    }
}

/// Example getting a task status from svc-scheduler
async fn get_task_status_example(
    client: &GrpcClient<RpcServiceClient<Channel>>,
    task_id: i64,
) -> Option<TaskMetadata> {
    let request = TaskRequest { task_id };

    match client.get_task_status(request).await {
        Ok(response) => {
            let metadata = response.into_inner().task_metadata;
            println!("(main) RESPONSE={:?}", &metadata);
            metadata
        }
        Err(e) => {
            println!("(main) ERROR={:?}", e);
            None
        }
    }
}

/// Example cancelling an itinerary from svc-scheduler
async fn cancel_itinerary_example(
    client: &GrpcClient<RpcServiceClient<Channel>>,
    itinerary_id: &str,
) -> Option<i64> {
    let request = CancelItineraryRequest {
        priority: FlightPriority::Low.into(),
        itinerary_id: itinerary_id.to_string(),
    };

    match client.cancel_itinerary(request).await {
        Ok(response) => {
            let task_id = response.into_inner().task_id;
            println!("(main) RESPONSE={:?}", &task_id);
            Some(task_id)
        }
        Err(e) => {
            println!("(main) ERROR={:?}", e);
            None
        }
    }
}

/// Example cancelling a task from svc-scheduler
async fn cancel_task_example(
    client: &GrpcClient<RpcServiceClient<Channel>>,
    task_id: i64,
) -> Option<TaskResponse> {
    let request = TaskRequest { task_id };

    match client.cancel_task(request).await {
        Ok(response) => {
            let response = response.into_inner();
            println!("(main) RESPONSE={:?}", &task_id);
            Some(response)
        }
        Err(e) => {
            println!("(main) ERROR={:?}", e);
            None
        }
    }
}

/// Example svc-scheduler-client-grpc
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    let client = SchedulerClient::new_client(&host, port, "scheduler");
    println!("Client created");
    println!(
        "NOTE: Ensure the server is running on {} or this example will fail.",
        client.get_address()
    );

    //
    // is_ready endpoint
    //
    let ready = client.is_ready(ReadyRequest {}).await?.into_inner();
    assert_eq!(ready.ready, true);

    //
    // query_itinerary endpoint
    //
    // let Some(mut itineraries) = query_itinerary_example(&client).await else {
    //     panic!("(main) Example failed; query itinerary failed.");
    // };

    //
    // create_itinerary endpoint
    // should return a ticket number
    //
    let itinerary = Itinerary {
        flight_plans: vec![scheduler_storage::flight_plan::Data {
            origin_vertiport_id: Some(uuid::Uuid::new_v4().to_string()),
            origin_vertipad_id: uuid::Uuid::new_v4().to_string(),
            origin_timeslot_start: Some((Utc::now() + chrono::Duration::minutes(10)).into()),
            origin_timeslot_end: Some((Utc::now() + chrono::Duration::minutes(11)).into()),
            target_vertiport_id: Some(uuid::Uuid::new_v4().to_string()),
            target_vertipad_id: uuid::Uuid::new_v4().to_string(),
            target_timeslot_start: Some((Utc::now() + chrono::Duration::minutes(30)).into()),
            target_timeslot_end: Some((Utc::now() + chrono::Duration::minutes(31)).into()),
            vehicle_id: uuid::Uuid::new_v4().to_string(),
            ..Default::default()
        }],
    };

    let Some(task_id) = create_itinerary_example(&client, &itinerary).await else {
        panic!("(main) Example failed; create itinerary failed.");
    };

    //
    // Get task status endpoint
    //
    let Some(_) = get_task_status_example(&client, task_id).await else {
        panic!("(main) Example failed; get task status failed.");
    };

    //
    // Cancel Itinerary endpoint
    //
    let itinerary_id = uuid::Uuid::new_v4().to_string();
    let Some(task_id) = cancel_itinerary_example(&client, &itinerary_id).await else {
        panic!("(main) Example failed; cancel itinerary failed.");
    };

    // //
    // // Cancel Task Endpoint
    // //
    // let Some(_) = cancel_task_example(&client, task_id).await else {
    //     panic!("(main) Example failed; cancel task failed.");
    // };

    Ok(())
}
