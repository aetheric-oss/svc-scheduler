/// QueryFlightRequest
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryFlightRequest {
    /// is_cargo - true if cargo mission, false if people transport
    #[prost(bool, tag="1")]
    pub is_cargo: bool,
    /// persons - number of people for transport
    #[prost(uint32, tag="2")]
    pub persons: u32,
    /// weight in grams
    #[prost(uint32, tag="3")]
    pub weight_grams: u32,
    /// requested preferred time of flight
    #[prost(message, optional, tag="4")]
    pub requested_time: ::core::option::Option<::prost_types::Timestamp>,
    /// longitude
    #[prost(float, tag="5")]
    pub longitude: f32,
    /// latitude
    #[prost(float, tag="6")]
    pub latitude: f32,
}
/// QueryFlightPlan
#[derive(Eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryFlightPlan {
    /// id of the flight
    #[prost(uint32, tag="1")]
    pub id: u32,
    /// pilot_id
    #[prost(uint32, tag="2")]
    pub pilot_id: u32,
    /// aircraft_id
    #[prost(uint32, tag="3")]
    pub aircraft_id: u32,
    /// cargo
    #[prost(uint32, repeated, tag="4")]
    pub cargo: ::prost::alloc::vec::Vec<u32>,
    /// weather_conditions
    #[prost(string, tag="5")]
    pub weather_conditions: ::prost::alloc::string::String,
    /// vertiport_id_departure
    #[prost(uint32, tag="6")]
    pub vertiport_id_departure: u32,
    /// pad_id_departure
    #[prost(uint32, tag="7")]
    pub pad_id_departure: u32,
    /// vertiport_id_destination
    #[prost(uint32, tag="8")]
    pub vertiport_id_destination: u32,
    /// pad_id_destination
    #[prost(uint32, tag="9")]
    pub pad_id_destination: u32,
    /// estimated_departure
    #[prost(message, optional, tag="10")]
    pub estimated_departure: ::core::option::Option<::prost_types::Timestamp>,
    /// estimated_arrival
    #[prost(message, optional, tag="11")]
    pub estimated_arrival: ::core::option::Option<::prost_types::Timestamp>,
    /// actual_departure
    #[prost(message, optional, tag="12")]
    pub actual_departure: ::core::option::Option<::prost_types::Timestamp>,
    /// actual_arrival
    #[prost(message, optional, tag="13")]
    pub actual_arrival: ::core::option::Option<::prost_types::Timestamp>,
    /// flight_release_approval
    #[prost(message, optional, tag="14")]
    pub flight_release_approval: ::core::option::Option<::prost_types::Timestamp>,
    /// flight_plan_submitted
    #[prost(message, optional, tag="15")]
    pub flight_plan_submitted: ::core::option::Option<::prost_types::Timestamp>,
    /// flightStatus
    #[prost(enumeration="FlightStatus", tag="16")]
    pub flight_status: i32,
    /// flightPriority
    #[prost(enumeration="FlightPriority", tag="17")]
    pub flight_priority: i32,
}
/// QueryFlightResponse
#[derive(Eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryFlightResponse {
    /// array/vector of flight items
    #[prost(message, repeated, tag="1")]
    pub flights: ::prost::alloc::vec::Vec<QueryFlightPlan>,
}
/// Id type for passing id only requests
#[derive(Eq, Copy)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Id {
    /// id
    #[prost(uint32, tag="1")]
    pub id: u32,
}
/// ConfirmFlightResponse
#[derive(Eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConfirmFlightResponse {
    /// id
    #[prost(uint32, tag="1")]
    pub id: u32,
    /// indicates if confirmation was successful
    #[prost(bool, tag="2")]
    pub confirmed: bool,
    /// time when the flight was confirmed
    #[prost(message, optional, tag="3")]
    pub confirmation_time: ::core::option::Option<::prost_types::Timestamp>,
}
/// CancelFlightResponse
#[derive(Eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelFlightResponse {
    /// id
    #[prost(uint32, tag="1")]
    pub id: u32,
    /// indicates if cancellation was successful
    #[prost(bool, tag="2")]
    pub cancelled: bool,
    /// time when the flight was cancelled
    #[prost(message, optional, tag="3")]
    pub cancellation_time: ::core::option::Option<::prost_types::Timestamp>,
    /// reason of cancellation
    #[prost(string, tag="4")]
    pub reason: ::prost::alloc::string::String,
}
/// Ready Request
///
/// No arguments
#[derive(Eq, Copy)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadyRequest {
}
/// Ready Response
#[derive(Eq, Copy)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadyResponse {
    /// ready
    #[prost(bool, tag="1")]
    pub ready: bool,
}
/// Flight Status Enum
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum FlightStatus {
    /// READY
    Ready = 0,
    /// BOARDING
    Boarding = 1,
    /// IN_FLIGHT
    InFlight = 3,
    /// FINISHED
    Finished = 4,
    /// CANCELLED
    Cancelled = 5,
    /// DRAFT
    Draft = 6,
}
impl FlightStatus {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            FlightStatus::Ready => "READY",
            FlightStatus::Boarding => "BOARDING",
            FlightStatus::InFlight => "IN_FLIGHT",
            FlightStatus::Finished => "FINISHED",
            FlightStatus::Cancelled => "CANCELLED",
            FlightStatus::Draft => "DRAFT",
        }
    }
}
/// Flight Priority Enum
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum FlightPriority {
    /// LOW
    Low = 0,
    /// HIGH
    High = 1,
    /// EMERGENCY
    Emergency = 2,
}
impl FlightPriority {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            FlightPriority::Low => "LOW",
            FlightPriority::High => "HIGH",
            FlightPriority::Emergency => "EMERGENCY",
        }
    }
}
/// Generated client implementations.
pub mod scheduler_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    ///Scheduler service
    #[derive(Debug, Clone)]
    pub struct SchedulerClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl SchedulerClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> SchedulerClient<T>
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
        ) -> SchedulerClient<InterceptedService<T, F>>
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
            SchedulerClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn query_flight(
            &mut self,
            request: impl tonic::IntoRequest<super::QueryFlightRequest>,
        ) -> Result<tonic::Response<super::QueryFlightResponse>, tonic::Status> {
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
                "/svc_scheduler.Scheduler/queryFlight",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn confirm_flight(
            &mut self,
            request: impl tonic::IntoRequest<super::Id>,
        ) -> Result<tonic::Response<super::ConfirmFlightResponse>, tonic::Status> {
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
                "/svc_scheduler.Scheduler/confirmFlight",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn cancel_flight(
            &mut self,
            request: impl tonic::IntoRequest<super::Id>,
        ) -> Result<tonic::Response<super::CancelFlightResponse>, tonic::Status> {
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
                "/svc_scheduler.Scheduler/cancelFlight",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn is_ready(
            &mut self,
            request: impl tonic::IntoRequest<super::ReadyRequest>,
        ) -> Result<tonic::Response<super::ReadyResponse>, tonic::Status> {
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
                "/svc_scheduler.Scheduler/isReady",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
}
