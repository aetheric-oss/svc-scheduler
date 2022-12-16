/// QueryFlightRequest
#[derive(Eq)]
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
    /// requested preferred time of departure - if not set, then arrival time is used; if both set, then departure time is used
    #[prost(message, optional, tag = "4")]
    pub departure_time: ::core::option::Option<::prost_types::Timestamp>,
    /// requested preferred time of arrival - if not set, then departure time is used; if both set, then departure time is used
    #[prost(message, optional, tag = "5")]
    pub arrival_time: ::core::option::Option<::prost_types::Timestamp>,
    /// vertiport_depart_id
    #[prost(string, tag = "6")]
    pub vertiport_depart_id: ::prost::alloc::string::String,
    /// vertiport_depart_id
    #[prost(string, tag = "7")]
    pub vertiport_arrive_id: ::prost::alloc::string::String,
}
/// QueryFlightPlan
#[derive(Eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryFlightPlan {
    /// id of the flight
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String,
    /// pilot_id
    #[prost(string, tag = "2")]
    pub pilot_id: ::prost::alloc::string::String,
    /// vehicle_id
    #[prost(string, tag = "3")]
    pub vehicle_id: ::prost::alloc::string::String,
    /// cargo
    #[prost(uint32, repeated, tag = "4")]
    pub cargo: ::prost::alloc::vec::Vec<u32>,
    /// weather_conditions
    #[prost(string, tag = "5")]
    pub weather_conditions: ::prost::alloc::string::String,
    /// vertiport_depart_id
    #[prost(string, tag = "6")]
    pub vertiport_depart_id: ::prost::alloc::string::String,
    /// pad_depart_id
    #[prost(string, tag = "7")]
    pub pad_depart_id: ::prost::alloc::string::String,
    /// vertiport_arrive_id
    #[prost(string, tag = "8")]
    pub vertiport_arrive_id: ::prost::alloc::string::String,
    /// pad_arrive_id
    #[prost(string, tag = "9")]
    pub pad_arrive_id: ::prost::alloc::string::String,
    /// estimated_departure
    #[prost(message, optional, tag = "10")]
    pub estimated_departure: ::core::option::Option<::prost_types::Timestamp>,
    /// estimated_arrival
    #[prost(message, optional, tag = "11")]
    pub estimated_arrival: ::core::option::Option<::prost_types::Timestamp>,
    /// actual_departure
    #[prost(message, optional, tag = "12")]
    pub actual_departure: ::core::option::Option<::prost_types::Timestamp>,
    /// actual_arrival
    #[prost(message, optional, tag = "13")]
    pub actual_arrival: ::core::option::Option<::prost_types::Timestamp>,
    /// flight_release_approval
    #[prost(message, optional, tag = "14")]
    pub flight_release_approval: ::core::option::Option<::prost_types::Timestamp>,
    /// flight_plan_submitted
    #[prost(message, optional, tag = "15")]
    pub flight_plan_submitted: ::core::option::Option<::prost_types::Timestamp>,
    /// flightStatus
    #[prost(enumeration = "FlightStatus", tag = "16")]
    pub flight_status: i32,
    /// flightPriority
    #[prost(enumeration = "FlightPriority", tag = "17")]
    pub flight_priority: i32,
    /// estimated distance in meters
    #[prost(uint32, tag = "18")]
    pub estimated_distance: u32,
}
/// QueryFlightResponse
#[derive(Eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryFlightResponse {
    /// array/vector of flight items
    #[prost(message, repeated, tag = "1")]
    pub flights: ::prost::alloc::vec::Vec<QueryFlightPlan>,
}
/// Id type for passing id only requests
#[derive(Eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Id {
    /// id
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String,
}
/// ConfirmFlightResponse
#[derive(Eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConfirmFlightResponse {
    /// id
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String,
    /// indicates if confirmation was successful
    #[prost(bool, tag = "2")]
    pub confirmed: bool,
    /// time when the flight was confirmed
    #[prost(message, optional, tag = "3")]
    pub confirmation_time: ::core::option::Option<::prost_types::Timestamp>,
}
/// CancelFlightResponse
#[derive(Eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelFlightResponse {
    /// id
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String,
    /// indicates if cancellation was successful
    #[prost(bool, tag = "2")]
    pub cancelled: bool,
    /// time when the flight was cancelled
    #[prost(message, optional, tag = "3")]
    pub cancellation_time: ::core::option::Option<::prost_types::Timestamp>,
    /// reason of cancellation
    #[prost(string, tag = "4")]
    pub reason: ::prost::alloc::string::String,
}
/// Ready Request
///
/// No arguments
#[derive(Eq, Copy)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadyRequest {}
/// Ready Response
#[derive(Eq, Copy)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadyResponse {
    /// ready
    #[prost(bool, tag = "1")]
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
/// Generated server implementations.
pub mod scheduler_rpc_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    ///Generated trait containing gRPC methods that should be implemented for use with SchedulerRpcServer.
    #[async_trait]
    pub trait SchedulerRpc: Send + Sync + 'static {
        async fn query_flight(
            &self,
            request: tonic::Request<super::QueryFlightRequest>,
        ) -> Result<tonic::Response<super::QueryFlightResponse>, tonic::Status>;
        async fn confirm_flight(
            &self,
            request: tonic::Request<super::Id>,
        ) -> Result<tonic::Response<super::ConfirmFlightResponse>, tonic::Status>;
        async fn cancel_flight(
            &self,
            request: tonic::Request<super::Id>,
        ) -> Result<tonic::Response<super::CancelFlightResponse>, tonic::Status>;
        async fn is_ready(
            &self,
            request: tonic::Request<super::ReadyRequest>,
        ) -> Result<tonic::Response<super::ReadyResponse>, tonic::Status>;
    }
    ///Scheduler service
    #[derive(Debug)]
    pub struct SchedulerRpcServer<T: SchedulerRpc> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: SchedulerRpc> SchedulerRpcServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for SchedulerRpcServer<T>
    where
        T: SchedulerRpc,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/grpc.SchedulerRpc/queryFlight" => {
                    #[allow(non_camel_case_types)]
                    struct queryFlightSvc<T: SchedulerRpc>(pub Arc<T>);
                    impl<
                        T: SchedulerRpc,
                    > tonic::server::UnaryService<super::QueryFlightRequest>
                    for queryFlightSvc<T> {
                        type Response = super::QueryFlightResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::QueryFlightRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).query_flight(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = queryFlightSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/grpc.SchedulerRpc/confirmFlight" => {
                    #[allow(non_camel_case_types)]
                    struct confirmFlightSvc<T: SchedulerRpc>(pub Arc<T>);
                    impl<T: SchedulerRpc> tonic::server::UnaryService<super::Id>
                    for confirmFlightSvc<T> {
                        type Response = super::ConfirmFlightResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::Id>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).confirm_flight(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = confirmFlightSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/grpc.SchedulerRpc/cancelFlight" => {
                    #[allow(non_camel_case_types)]
                    struct cancelFlightSvc<T: SchedulerRpc>(pub Arc<T>);
                    impl<T: SchedulerRpc> tonic::server::UnaryService<super::Id>
                    for cancelFlightSvc<T> {
                        type Response = super::CancelFlightResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::Id>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).cancel_flight(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = cancelFlightSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/grpc.SchedulerRpc/isReady" => {
                    #[allow(non_camel_case_types)]
                    struct isReadySvc<T: SchedulerRpc>(pub Arc<T>);
                    impl<
                        T: SchedulerRpc,
                    > tonic::server::UnaryService<super::ReadyRequest>
                    for isReadySvc<T> {
                        type Response = super::ReadyResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ReadyRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).is_ready(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = isReadySvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        Ok(
                            http::Response::builder()
                                .status(200)
                                .header("grpc-status", "12")
                                .header("content-type", "application/grpc")
                                .body(empty_body())
                                .unwrap(),
                        )
                    })
                }
            }
        }
    }
    impl<T: SchedulerRpc> Clone for SchedulerRpcServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
            }
        }
    }
    impl<T: SchedulerRpc> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: SchedulerRpc> tonic::server::NamedService for SchedulerRpcServer<T> {
        const NAME: &'static str = "grpc.SchedulerRpc";
    }
}
