//! gRPC server implementation

pub mod grpc_server {
    #![allow(unused_qualifications, missing_docs)]
    tonic::include_proto!("grpc");
}
pub use grpc_server::rpc_service_server::{RpcService, RpcServiceServer};
pub use grpc_server::{
    CancelItineraryRequest, CreateItineraryRequest, Itinerary, QueryFlightRequest,
    QueryFlightResponse, ReadyRequest, ReadyResponse, TaskAction, TaskMetadata, TaskRequest,
    TaskResponse, TaskStatus,
};

use crate::shutdown_signal;
use crate::Config;

use std::fmt::Debug;
use std::net::SocketAddr;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

#[cfg(feature = "stub_server")]
use rand::Rng;

/// struct to implement the gRPC server functions
#[derive(Debug, Copy, Clone, Default)]
pub struct ServerImpl {}

#[cfg(not(feature = "stub_server"))]
#[tonic::async_trait]
impl RpcService for ServerImpl {
    ///finds the first possible flight for customer location, flight type and requested time.
    /// Returns possible itineraries which can be used to create an itinerary.
    async fn query_flight(
        &self,
        request: Request<QueryFlightRequest>,
    ) -> Result<Response<QueryFlightResponse>, Status> {
        grpc_info!("(query_flight) scheduler server.");
        grpc_debug!("(query_flight) request: {:?}", request);

        let request = request.into_inner();
        let res = super::api::query_flight::query_flight(request).await;
        if let Err(e) = res {
            grpc_error!("(query_flight) error: {}", e);
            return Err(e);
        }

        res
    }

    /// Creates an itinerary given a list of flight plans, if possible.
    async fn create_itinerary(
        &self,
        request: Request<CreateItineraryRequest>,
    ) -> Result<Response<TaskResponse>, Status>
    where
        Self: Send,
    {
        grpc_info!("(create_itinerary) scheduler server.");
        grpc_debug!("(create_itinerary) request: {:?}", request);

        let request = request.into_inner();
        let response = super::api::create::create_itinerary(request).await;
        match response {
            Ok(response) => Ok(Response::new(response)),
            Err(e) => {
                grpc_error!("(create_itinerary) error: {}", e);
                Err(Status::internal("Could not create itinerary."))
            }
        }
    }

    /// Cancels the itinerary by id.
    async fn cancel_itinerary(
        &self,
        request: Request<CancelItineraryRequest>,
    ) -> Result<Response<TaskResponse>, Status>
    where
        Self: Send,
    {
        grpc_info!("(cancel_itinerary) scheduler server.");
        grpc_debug!("(cancel_itinerary) request: {:?}", request);

        let request = request.into_inner();
        let response = super::api::cancel::cancel_itinerary(request).await;

        match response {
            Ok(response) => Ok(Response::new(response)),
            Err(e) => {
                grpc_error!("(cancel_itinerary) error: {}", e);
                Err(Status::internal("Could not cancel itinerary."))
            }
        }
    }

    /// Cancels a scheduler task before it can be processed
    async fn cancel_task(
        &self,
        request: Request<TaskRequest>,
    ) -> Result<Response<TaskResponse>, Status>
    where
        Self: Send,
    {
        grpc_info!("(cancel_task) scheduler server.");
        grpc_debug!("(cancel_task) request: {:?}", request);
        let request = request.into_inner();

        match crate::tasks::cancel_task(request.task_id).await {
            Ok(()) => {
                let response = TaskResponse {
                    task_id: request.task_id,
                    task_metadata: None,
                };

                Ok(Response::new(response))
            }
            Err(e) => {
                grpc_error!("(cancel_task) error: {}", e);
                Err(Status::internal("Could not cancel task."))
            }
        }
    }

    /// Returns the status of a scheduler task
    async fn get_task_status(
        &self,
        request: Request<TaskRequest>,
    ) -> Result<Response<TaskResponse>, Status>
    where
        Self: Send,
    {
        grpc_info!("(get_task_status) scheduler server.");
        grpc_debug!("(get_task_status) request: {:?}", request);
        let request = request.into_inner();
        match crate::tasks::get_task_status(request.task_id).await {
            Ok(task_metadata) => {
                let response = TaskResponse {
                    task_id: request.task_id,
                    task_metadata: Some(task_metadata),
                };

                Ok(Response::new(response))
            }
            Err(e) => {
                grpc_error!("(get_task_status) error: {}", e);
                Err(Status::internal("Could not get task status."))
            }
        }
    }

    /// Returns ready:true when service is available
    async fn is_ready(
        &self,
        request: Request<ReadyRequest>,
    ) -> Result<Response<ReadyResponse>, Status> {
        grpc_info!("(is_ready) scheduler server.");
        grpc_debug!("(is_ready) request: {:?}", request);
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }
}

/// Starts the grpc servers for this microservice using the provided configuration
///
/// # Example:
/// ```
/// use svc_scheduler::grpc::server::grpc_server;
/// use svc_scheduler::Config;
/// async fn example() -> Result<(), tokio::task::JoinError> {
///     let config = Config::default();
///     tokio::spawn(grpc_server(config, None)).await;
///     Ok(())
/// }
/// ```
pub async fn grpc_server(config: Config, shutdown_rx: Option<tokio::sync::oneshot::Receiver<()>>) {
    grpc_debug!("(grpc_server) entry.");

    // Grpc Server
    let grpc_port = config.docker_port_grpc;
    let full_grpc_addr: SocketAddr = match format!("[::]:{}", grpc_port).parse() {
        Ok(addr) => addr,
        Err(e) => {
            grpc_error!("(grpc_server) Failed to parse gRPC address: {}", e);
            return;
        }
    };

    let imp = ServerImpl {};

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<RpcServiceServer<ServerImpl>>()
        .await;

    //start server
    grpc_info!(
        "(grpc_server) Starting gRPC services on: {}.",
        full_grpc_addr
    );
    match Server::builder()
        .add_service(health_service)
        .add_service(RpcServiceServer::new(imp))
        .serve_with_shutdown(full_grpc_addr, shutdown_signal("grpc", shutdown_rx))
        .await
    {
        Ok(_) => grpc_info!("(grpc_server) gRPC server running at: {}.", full_grpc_addr),
        Err(e) => {
            grpc_error!("(grpc_server) could not start gRPC server: {}", e);
        }
    };
}

#[cfg(feature = "stub_server")]
#[tonic::async_trait]
impl RpcService for ServerImpl {
    /// Calculates possible itineraries given dates, times, locations, and other constraints.
    async fn query_flight(
        &self,
        request: Request<QueryFlightRequest>,
    ) -> Result<Response<QueryFlightResponse>, Status> {
        grpc_warn!("(query_flight MOCK) scheduler server.");
        grpc_debug!("(query_flight MOCK) request: {:?}", request);
        let flight_plan_data =
            svc_storage_client_grpc::prelude::flight_plan::mock::get_future_data_obj();

        let itineraries = vec![Itinerary {
            flight_plans: vec![flight_plan_data],
        }];

        Ok(tonic::Response::new(QueryFlightResponse { itineraries }))
    }

    /// Creates an itinerary given a list of proposed flight plans, if possible.
    async fn create_itinerary(
        &self,
        request: Request<CreateItineraryRequest>,
    ) -> Result<Response<TaskResponse>, Status> {
        grpc_warn!("(create_itinerary MOCK) scheduler server.");
        grpc_debug!("(create_itinerary MOCK) request: {:?}", request);
        let mut rng = rand::thread_rng();
        Ok(tonic::Response::new(TaskResponse {
            task_id: rng.gen_range(0..1000),
            task_metadata: Some(TaskMetadata {
                status: TaskStatus::Queued as i32,
                action: TaskAction::CreateItinerary as i32,
                ..Default::default()
            }),
        }))
    }

    /// Cancels the itinerary by id.
    async fn cancel_itinerary(
        &self,
        request: Request<CancelItineraryRequest>,
    ) -> Result<Response<TaskResponse>, Status> {
        grpc_warn!("(cancel_itinerary MOCK) scheduler server.");
        grpc_debug!("(cancel_itinerary MOCK) request: {:?}", request);
        let mut rng = rand::thread_rng();
        Ok(tonic::Response::new(TaskResponse {
            task_id: rng.gen_range(0..1000),
            task_metadata: Some(TaskMetadata {
                status: TaskStatus::Queued as i32,
                action: TaskAction::CancelItinerary as i32,
                ..Default::default()
            }),
        }))
    }

    /// Cancels a scheduler task
    async fn cancel_task(
        &self,
        request: Request<TaskRequest>,
    ) -> Result<Response<TaskResponse>, Status> {
        grpc_warn!("(cancel_task MOCK) scheduler server.");
        grpc_debug!("(cancel_task MOCK) request: {:?}", request);
        let response = TaskResponse {
            task_id: request.into_inner().task_id,
            task_metadata: None,
        };
        Ok(Response::new(response))
    }

    /// Returns the status of a scheduler task
    async fn get_task_status(
        &self,
        request: Request<TaskRequest>,
    ) -> Result<Response<TaskResponse>, Status> {
        grpc_warn!("(get_task_status MOCK) scheduler server.");
        grpc_debug!("(get_task_status MOCK) request: {:?}", request);

        let response = TaskResponse {
            task_id: request.into_inner().task_id,
            task_metadata: Some(TaskMetadata {
                status: TaskStatus::Queued as i32,
                ..Default::default()
            }),
        };

        Ok(Response::new(response))
    }

    /// Returns ready:true when service is available
    async fn is_ready(
        &self,
        request: Request<ReadyRequest>,
    ) -> Result<Response<ReadyResponse>, Status> {
        grpc_warn!("(is_ready MOCK) scheduler server.");
        grpc_debug!("(is_ready MOCK) request: {:?}", request);
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }
}
