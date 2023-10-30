/// QueryFlightRequest
#[derive(Eq)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryFlightRequest {
    /// is_cargo - true if cargo mission, false if people transport
    #[prost(bool, tag = "1")]
    pub is_cargo: bool,
    /// persons - number of people for transport
    #[prost(uint32, optional, tag = "2")]
    pub persons: ::core::option::Option<u32>,
    /// weight in grams
    #[prost(uint32, optional, tag = "3")]
    pub weight_grams: ::core::option::Option<u32>,
    /// requested earliest time of departure - beginning of the time window in which we search for a flight
    #[prost(message, optional, tag = "4")]
    pub earliest_departure_time: ::core::option::Option<::prost_wkt_types::Timestamp>,
    /// requested preferred time of arrival - end of the time window in which we search for a flight
    #[prost(message, optional, tag = "5")]
    pub latest_arrival_time: ::core::option::Option<::prost_wkt_types::Timestamp>,
    /// departure vertiport ID
    #[prost(string, tag = "6")]
    pub origin_vertiport_id: ::prost::alloc::string::String,
    /// arrival vertiport ID
    #[prost(string, tag = "7")]
    pub target_vertiport_id: ::prost::alloc::string::String,
    /// Flight priority (from svc-storage)
    #[prost(
        enumeration = "::svc_storage_client_grpc::prelude::flight_plan::FlightPriority",
        tag = "8"
    )]
    pub priority: i32,
}
/// Create an itinerary by providing possible flight plan data
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateItineraryRequest {
    /// Flight priority (from svc-storage)
    #[prost(
        enumeration = "::svc_storage_client_grpc::prelude::flight_plan::FlightPriority",
        tag = "1"
    )]
    pub priority: i32,
    /// Flight plans to be considered
    #[prost(message, repeated, tag = "2")]
    pub flight_plans: ::prost::alloc::vec::Vec<
        ::svc_storage_client_grpc::prelude::flight_plan::Data,
    >,
}
/// Cancel an itinerary by ID
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelItineraryRequest {
    /// Priority of the cancellation task
    #[prost(
        enumeration = "::svc_storage_client_grpc::prelude::flight_plan::FlightPriority",
        tag = "1"
    )]
    pub priority: i32,
    /// Itinerary UUID
    #[prost(string, tag = "2")]
    pub itinerary_id: ::prost::alloc::string::String,
}
/// Itinerary includes id, flight plan and potential deadhead flights
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Itinerary {
    /// flight_plan
    #[prost(message, repeated, tag = "1")]
    pub flight_plans: ::prost::alloc::vec::Vec<
        ::svc_storage_client_grpc::prelude::flight_plan::Data,
    >,
}
/// QueryFlightResponse
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryFlightResponse {
    /// array/vector of itineraries items
    #[prost(message, repeated, tag = "1")]
    pub itineraries: ::prost::alloc::vec::Vec<Itinerary>,
}
/// Task-Related Messages
#[derive(Eq, Copy)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TaskRequest {
    /// Task ID
    #[prost(int64, tag = "1")]
    pub task_id: i64,
}
/// Response to Task-Related Requests
#[derive(Eq, Copy)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TaskResponse {
    /// Task ID
    #[prost(int64, tag = "1")]
    pub task_id: i64,
    /// Task Details
    #[prost(message, optional, tag = "2")]
    pub task_metadata: ::core::option::Option<TaskMetadata>,
}
/// Metadata for a Scheduler Task
#[derive(serde::Serialize, serde::Deserialize, Eq, Copy)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TaskMetadata {
    /// Task status
    #[prost(enumeration = "TaskStatus", tag = "1")]
    pub status: i32,
    /// Task status rationale
    #[prost(enumeration = "TaskStatusRationale", optional, tag = "2")]
    pub status_rationale: ::core::option::Option<i32>,
    /// Task action
    #[prost(enumeration = "TaskAction", tag = "3")]
    pub action: i32,
}
/// Ready Request
///
/// No arguments
#[derive(Eq, Copy)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadyRequest {}
/// Ready Response
#[derive(Eq, Copy)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadyResponse {
    /// ready
    #[prost(bool, tag = "1")]
    pub ready: bool,
}
/// The status of a scheduler task
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum TaskStatus {
    /// Queued
    Queued = 0,
    /// Complete
    Complete = 1,
    /// Rejected
    Rejected = 2,
    /// Not Found
    NotFound = 3,
}
impl TaskStatus {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            TaskStatus::Queued => "QUEUED",
            TaskStatus::Complete => "COMPLETE",
            TaskStatus::Rejected => "REJECTED",
            TaskStatus::NotFound => "NOT_FOUND",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "QUEUED" => Some(Self::Queued),
            "COMPLETE" => Some(Self::Complete),
            "REJECTED" => Some(Self::Rejected),
            "NOT_FOUND" => Some(Self::NotFound),
            _ => None,
        }
    }
}
/// Explanation for a task status
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum TaskStatusRationale {
    /// Client cancelled
    ClientCancelled = 0,
    /// Expired
    Expired = 1,
    /// Schedule conflict
    ScheduleConflict = 2,
    /// Itinerary ID not found
    ItineraryIdNotFound = 3,
    /// Priority change
    PriorityChange = 4,
    /// Internal Failure
    Internal = 5,
    /// Invalid Action
    InvalidAction = 6,
}
impl TaskStatusRationale {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            TaskStatusRationale::ClientCancelled => "CLIENT_CANCELLED",
            TaskStatusRationale::Expired => "EXPIRED",
            TaskStatusRationale::ScheduleConflict => "SCHEDULE_CONFLICT",
            TaskStatusRationale::ItineraryIdNotFound => "ITINERARY_ID_NOT_FOUND",
            TaskStatusRationale::PriorityChange => "PRIORITY_CHANGE",
            TaskStatusRationale::Internal => "INTERNAL",
            TaskStatusRationale::InvalidAction => "INVALID_ACTION",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "CLIENT_CANCELLED" => Some(Self::ClientCancelled),
            "EXPIRED" => Some(Self::Expired),
            "SCHEDULE_CONFLICT" => Some(Self::ScheduleConflict),
            "ITINERARY_ID_NOT_FOUND" => Some(Self::ItineraryIdNotFound),
            "PRIORITY_CHANGE" => Some(Self::PriorityChange),
            "INTERNAL" => Some(Self::Internal),
            "INVALID_ACTION" => Some(Self::InvalidAction),
            _ => None,
        }
    }
}
/// Types of scheduler tasks
#[derive(num_derive::FromPrimitive)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum TaskAction {
    /// Confirm itinerary
    CreateItinerary = 0,
    /// Cancel itinerary
    CancelItinerary = 1,
}
impl TaskAction {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            TaskAction::CreateItinerary => "CREATE_ITINERARY",
            TaskAction::CancelItinerary => "CANCEL_ITINERARY",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "CREATE_ITINERARY" => Some(Self::CreateItinerary),
            "CANCEL_ITINERARY" => Some(Self::CancelItinerary),
            _ => None,
        }
    }
}
/// Generated client implementations.
#[cfg(not(tarpaulin_include))]
pub mod rpc_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// Scheduler service
    #[derive(Debug, Clone)]
    pub struct RpcServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl RpcServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> RpcServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> RpcServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            RpcServiceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        pub async fn query_flight(
            &mut self,
            request: impl tonic::IntoRequest<super::QueryFlightRequest>,
        ) -> std::result::Result<
            tonic::Response<super::QueryFlightResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/grpc.RpcService/queryFlight",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("grpc.RpcService", "queryFlight"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn create_itinerary(
            &mut self,
            request: impl tonic::IntoRequest<super::CreateItineraryRequest>,
        ) -> std::result::Result<tonic::Response<super::TaskResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/grpc.RpcService/createItinerary",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("grpc.RpcService", "createItinerary"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn cancel_itinerary(
            &mut self,
            request: impl tonic::IntoRequest<super::CancelItineraryRequest>,
        ) -> std::result::Result<tonic::Response<super::TaskResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/grpc.RpcService/cancelItinerary",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("grpc.RpcService", "cancelItinerary"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn cancel_task(
            &mut self,
            request: impl tonic::IntoRequest<super::TaskRequest>,
        ) -> std::result::Result<tonic::Response<super::TaskResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/grpc.RpcService/cancelTask",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("grpc.RpcService", "cancelTask"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_task_status(
            &mut self,
            request: impl tonic::IntoRequest<super::TaskRequest>,
        ) -> std::result::Result<tonic::Response<super::TaskResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/grpc.RpcService/getTaskStatus",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("grpc.RpcService", "getTaskStatus"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn is_ready(
            &mut self,
            request: impl tonic::IntoRequest<super::ReadyRequest>,
        ) -> std::result::Result<tonic::Response<super::ReadyResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/grpc.RpcService/isReady");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("grpc.RpcService", "isReady"));
            self.inner.unary(req, path, codec).await
        }
    }
}
