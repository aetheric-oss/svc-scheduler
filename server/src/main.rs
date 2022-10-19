//! gRPC server implementation

///module svc_scheduler generated from svc-scheduler.proto
pub mod svc_scheduler {
    #![allow(unused_qualifications)]
    include!("svc_scheduler.rs");
}
mod queries;
mod router_utils;

use dotenv::dotenv;
use once_cell::sync::OnceCell;
use router_utils::{
    get_nearby_nodes, get_nearest_vertiports, get_route, init_router, Aircraft,
    NearbyLocationQuery, RouteQuery, SAN_FRANCISCO,
};
use std::env;
use svc_scheduler::scheduler_server::{Scheduler, SchedulerServer};
use svc_scheduler::{
    CancelFlightResponse, ConfirmFlightResponse, Id, QueryFlightRequest, QueryFlightResponse,
    ReadyRequest, ReadyResponse,
};
use svc_storage_client::svc_storage::storage_client::StorageClient;
use tonic::{transport::Server, Request, Response, Status};

use chrono::TimeZone;
use ordered_float::OrderedFloat;
use router::location::Location;
use rrule::{RRuleSet, Tz};

/// GRPC client for storage service -
/// it has to be cloned before each call as per https://github.com/hyperium/tonic/issues/285
pub static STORAGE_CLIENT: OnceCell<StorageClient<tonic::transport::Channel>> = OnceCell::new();

/// shorthand function to clone storage client
pub fn get_storage_client() -> StorageClient<tonic::transport::Channel> {
    STORAGE_CLIENT
        .get()
        .expect("Storage Client not initialized")
        .clone()
}

///Implementation of gRPC endpoints
#[derive(Debug, Default, Copy, Clone)]
pub struct SchedulerImpl {}

#[tonic::async_trait]
impl Scheduler for SchedulerImpl {
    ///finds the first possible flight for customer location, flight type and requested time.
    /// Returns draft QueryFlightPlan which can be confirmed or cancelled.
    async fn query_flight(
        &self,
        request: Request<QueryFlightRequest>,
    ) -> Result<Response<QueryFlightResponse>, Status> {
        queries::query_flight(request, get_storage_client()).await
    }

    ///Confirms the draft flight plan by id.
    async fn confirm_flight(
        &self,
        request: Request<Id>,
    ) -> Result<Response<ConfirmFlightResponse>, Status> {
        queries::confirm_flight(request, get_storage_client()).await
    }

    ///Cancels the draft flight plan by id.
    async fn cancel_flight(
        &self,
        request: Request<Id>,
    ) -> Result<Response<CancelFlightResponse>, Status> {
        queries::cancel_flight(request, get_storage_client()).await
    }

    /// Returns ready:true when service is available
    async fn is_ready(
        &self,
        _request: Request<ReadyRequest>,
    ) -> Result<Response<ReadyResponse>, Status> {
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }
}

fn test_router() {
    let nodes = get_nearby_nodes(NearbyLocationQuery {
        location: SAN_FRANCISCO,
        radius: 25.0,
        capacity: 20,
    });

    //println!("nodes: {:?}", nodes);
    let init_res = init_router(Aircraft::ArrowCargo);
    println!("init_res: {:?}", init_res);
    let src_location = Location {
        latitude: OrderedFloat(37.52123),
        longitude: OrderedFloat(-122.50892),
        altitude_meters: OrderedFloat(20.0),
    };
    let dst_location = Location {
        latitude: OrderedFloat(37.81032),
        longitude: OrderedFloat(-122.28432),
        altitude_meters: OrderedFloat(20.0),
    };
    let (src, dst) = get_nearest_vertiports(&src_location, &dst_location, nodes);
    println!("src: {:?}, dst: {:?}", src.location, dst.location);
    let route = get_route(RouteQuery {
        from: src,
        to: dst,
        aircraft: Aircraft::ArrowCargo,
    });
    println!("route: {:?}", route);
}

///Main entry point: starts gRPC Server on specified address and port
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    test_router();

    let rrule: RRuleSet = "DTSTART:20221018T190000Z\nRRULE:FREQ=DAILY"
        .parse()
        .unwrap();

    let after = Tz::UTC.ymd(2022, 11, 19).and_hms(19, 0, 0);
    let before = Tz::UTC.ymd(2022, 11, 19).and_hms(20, 0, 0);

    let (events, skipped) = rrule.before(before).after(after).all(10);
    println!("Events: {:?} {:?}", events, skipped);
    //parse socket address from env variable or take default value
    let address = match env::var("GRPC_SOCKET_ADDR") {
        Ok(val) => val,
        Err(_) => "[::1]:50051".to_string(), // default value
    };
    let addr = address.parse()?;
    let scheduler = SchedulerImpl::default();
    //initialize storage client here so it can be used in other methods
    STORAGE_CLIENT
        .set(StorageClient::connect("http://[::1]:50052").await?)
        .unwrap();

    //start server
    Server::builder()
        .add_service(SchedulerServer::new(scheduler))
        .serve(addr)
        .await?;
    println!("gRPC server running at: {}", address);

    Ok(())
}
