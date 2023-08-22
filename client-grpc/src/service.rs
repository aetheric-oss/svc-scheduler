//! Client Library: Client Functions, Structs, Traits

/// gRPC object traits to provide wrappers for grpc functions
#[tonic::async_trait]
pub trait Client<T>
where
    Self: Sized + lib_common::grpc::Client<T> + lib_common::grpc::ClientConnect<T>,
    T: Send + Clone,
{
    /// The type expected for ReadyRequest structs.
    type ReadyRequest;
    /// The type expected for ReadyResponse structs.
    type ReadyResponse;

    /// Returns a [`tonic::Response`] containing a [`ReadyResponse`](Self::ReadyResponse)
    /// Takes an [`ReadyRequest`](Self::ReadyRequest).
    ///
    /// # Errors
    ///
    /// Returns [`tonic::Status`] with [`tonic::Code::Unknown`] if the server is not ready.
    ///
    /// # Examples
    /// ```
    /// use lib_common::grpc::get_endpoint_from_env;
    /// use scheduler::{ReadyRequest, SchedulerClient};
    /// use svc_scheduler_client_grpc::prelude::*;
    ///
    /// async fn example () -> Result<(), Box<dyn std::error::Error>> {
    ///     let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    ///     let connection = SchedulerClient::new_client(&host, port, "scheduler");
    ///     let response = connection
    ///         .is_ready(ReadyRequest {})
    ///         .await?;
    ///     println!("RESPONSE={:?}", response.into_inner());
    ///     Ok(())
    /// }
    /// ```
    async fn is_ready(
        &self,
        request: Self::ReadyRequest,
    ) -> Result<tonic::Response<Self::ReadyResponse>, tonic::Status>;

    /// wrapper
    async fn query_flight(
        &self,
        request: super::QueryFlightRequest,
    ) -> Result<tonic::Response<super::QueryFlightResponse>, tonic::Status>;

    /// wrapper
    async fn confirm_itinerary(
        &self,
        request: super::ConfirmItineraryRequest,
    ) -> Result<tonic::Response<super::ConfirmItineraryResponse>, tonic::Status>;

    /// wrapper
    async fn cancel_itinerary(
        &self,
        request: super::Id,
    ) -> Result<tonic::Response<super::CancelItineraryResponse>, tonic::Status>;
}
