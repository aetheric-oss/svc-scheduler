use crate::svc_scheduler::{
    CancelFlightResponse, ConfirmFlightResponse, FlightPriority, FlightStatus, Id, QueryFlightPlan,
    QueryFlightRequest, QueryFlightResponse,
};
use std::time::SystemTime;
use svc_storage_client::svc_storage;
use svc_storage_client::svc_storage::storage_client::StorageClient;
use svc_storage_client::svc_storage::{AircraftFilter, FlightPlan, PilotFilter, VertiportFilter};
use tonic::{Request, Response, Status};

pub async fn query_flight(
    request: Request<QueryFlightRequest>,
    mut storage_client: StorageClient<tonic::transport::Channel>,
) -> Result<Response<QueryFlightResponse>, Status> {
    let r_aircrafts = storage_client
        .aircrafts(Request::new(AircraftFilter {}))
        .await?;
    let r_vertiports = storage_client
        .vertiports(Request::new(VertiportFilter {}))
        .await?;
    let r_pilots = storage_client.pilots(Request::new(PilotFilter {})).await?;
    let r_aircraft = storage_client
        .aircraft_by_id(Request::new(svc_storage::Id { id: 1 }))
        .await?;
    println!(
        "RESPONSE={:?}, {:?}, {:?}, {:?}",
        r_aircrafts.into_inner(),
        r_vertiports.into_inner(),
        r_pilots.into_inner(),
        r_aircraft.into_inner()
    );

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
    Ok(Response::new(response))
}

pub async fn confirm_flight(
    request: Request<Id>,
    mut storage_client: StorageClient<tonic::transport::Channel>,
) -> Result<Response<ConfirmFlightResponse>, Status> {
    let fp_id = request.into_inner().id;
    let fp = storage_client
        .update_flight_plan_by_id(Request::new(FlightPlan {
            id: fp_id,
            flight_status: FlightStatus::Ready as i32,
        }))
        .await?;
    let sys_time = SystemTime::now();
    let response = ConfirmFlightResponse {
        id: fp_id,
        confirmed: fp.into_inner().flight_status == FlightStatus::Ready as i32,
        confirmation_time: Some(prost_types::Timestamp::from(sys_time)),
    };
    Ok(Response::new(response))
}

pub async fn cancel_flight(
    request: Request<Id>,
    mut storage_client: StorageClient<tonic::transport::Channel>,
) -> Result<Response<CancelFlightResponse>, Status> {
    let fp_id = request.into_inner().id;
    let fp = storage_client
        .update_flight_plan_by_id(Request::new(FlightPlan {
            id: fp_id,
            flight_status: FlightStatus::Cancelled as i32,
        }))
        .await?;
    let sys_time = SystemTime::now();
    let response = CancelFlightResponse {
        id: fp_id,
        cancelled: fp.into_inner().flight_status == FlightStatus::Cancelled as i32,
        cancellation_time: Some(prost_types::Timestamp::from(sys_time)),
        reason: "user cancelled".into(),
    };
    Ok(Response::new(response))
}
