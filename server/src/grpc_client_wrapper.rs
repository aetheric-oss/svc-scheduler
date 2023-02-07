use async_trait::async_trait;
use svc_compliance_client_grpc::client::compliance_rpc_client::ComplianceRpcClient;
use svc_compliance_client_grpc::client::{
    FlightPlanRequest, FlightPlanResponse, FlightReleaseRequest, FlightReleaseResponse,
};
use svc_storage_client_grpc::client::vehicle_rpc_client::VehicleRpcClient;

use svc_storage_client_grpc::client::{AdvancedSearchFilter, Id, SearchFilter, Vehicles};
use svc_storage_client_grpc::flight_plan::{
    Data as FlightPlanData, List as FlightPlans, Object as FlightPlan, Response as FPResponse,
    UpdateObject as UpdateFlightPlan,
};
use svc_storage_client_grpc::vertiport::{List as Vertiports, Object as Vertiport};
use svc_storage_client_grpc::{FlightPlanClient, VertipadClient, VertiportClient};
use tonic::transport::Channel;
use tonic::{Request, Response, Status};

fn storage_err_msg() -> Status {
    Status::internal("Storage client not initialized")
}

fn compliance_err_msg() -> Status {
    Status::internal("Compliance client not initialized")
}

#[derive(Debug)]
pub struct GRPCClients {
    pub flight_plan_client: FlightPlanClient<Channel>,
    pub vertiport_client: VertiportClient<Channel>,
    pub vertipad_client: VertipadClient<Channel>,
    pub vehicle_client: VehicleRpcClient<Channel>,
    pub compliance_client: ComplianceRpcClient<Channel>,
}

#[async_trait]
pub trait StorageClientWrapperTrait {
    async fn vertiports(
        &self,
        request: Request<AdvancedSearchFilter>,
    ) -> Result<Response<Vertiports>, Status>;
    async fn vertiport_by_id(&self, request: Request<Id>) -> Result<Response<Vertiport>, Status>;
    async fn flight_plan_by_id(&self, request: Request<Id>)
        -> Result<Response<FlightPlan>, Status>;
    async fn flight_plans(
        &self,
        request: Request<AdvancedSearchFilter>,
    ) -> Result<Response<FlightPlans>, Status>;
    async fn insert_flight_plan(
        &self,
        request: Request<FlightPlanData>,
    ) -> Result<Response<FPResponse>, Status>;
    async fn update_flight_plan(
        &self,
        request: Request<UpdateFlightPlan>,
    ) -> Result<Response<FPResponse>, Status>;
    async fn vehicles(&self, request: Request<SearchFilter>) -> Result<Response<Vehicles>, Status>;
}

#[async_trait]
pub trait ComplianceClientWrapperTrait {
    async fn submit_flight_plan(
        &self,
        request: Request<FlightPlanRequest>,
    ) -> Result<Response<FlightPlanResponse>, Status>;
    async fn request_flight_release(
        &self,
        request: Request<FlightReleaseRequest>,
    ) -> Result<Response<FlightReleaseResponse>, Status>;
}

#[derive(Debug)]
pub struct StorageClientWrapper {
    pub grpc_clients: Option<GRPCClients>,
}

#[derive(Debug)]
pub struct ComplianceClientWrapper {
    pub grpc_clients: Option<GRPCClients>,
}

#[async_trait]
impl StorageClientWrapperTrait for StorageClientWrapper {
    async fn vertiports(
        &self,
        request: Request<AdvancedSearchFilter>,
    ) -> Result<Response<Vertiports>, Status> {
        let mut vertiport_client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(storage_err_msg)
            .unwrap()
            .vertiport_client
            .clone();
        vertiport_client.search(request).await
    }

    async fn vertiport_by_id(&self, request: Request<Id>) -> Result<Response<Vertiport>, Status> {
        let mut vertiport_client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(storage_err_msg)
            .unwrap()
            .vertiport_client
            .clone();
        vertiport_client.get_by_id(request).await
    }

    async fn flight_plan_by_id(
        &self,
        request: Request<Id>,
    ) -> Result<Response<FlightPlan>, Status> {
        let mut fp_client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(storage_err_msg)
            .unwrap()
            .flight_plan_client
            .clone();
        fp_client.get_by_id(request).await
    }

    async fn flight_plans(
        &self,
        request: Request<AdvancedSearchFilter>,
    ) -> Result<Response<FlightPlans>, Status> {
        let mut fp_client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(storage_err_msg)
            .unwrap()
            .flight_plan_client
            .clone();
        fp_client.search(request).await
    }

    async fn insert_flight_plan(
        &self,
        request: Request<FlightPlanData>,
    ) -> Result<Response<FPResponse>, Status> {
        let mut fp_client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(storage_err_msg)
            .unwrap()
            .flight_plan_client
            .clone();
        fp_client.insert(request).await
    }

    async fn update_flight_plan(
        &self,
        request: Request<UpdateFlightPlan>,
    ) -> Result<Response<FPResponse>, Status> {
        let mut fp_client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(storage_err_msg)
            .unwrap()
            .flight_plan_client
            .clone();
        fp_client.update(request).await
    }

    async fn vehicles(&self, request: Request<SearchFilter>) -> Result<Response<Vehicles>, Status> {
        let mut vehicle_client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(storage_err_msg)
            .unwrap()
            .vehicle_client
            .clone();
        vehicle_client.vehicles(request).await
    }
}

#[async_trait]
impl ComplianceClientWrapperTrait for ComplianceClientWrapper {
    async fn submit_flight_plan(
        &self,
        request: Request<FlightPlanRequest>,
    ) -> Result<Response<FlightPlanResponse>, Status> {
        let mut compliance_client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(compliance_err_msg)
            .unwrap()
            .compliance_client
            .clone();
        compliance_client.submit_flight_plan(request).await
    }

    async fn request_flight_release(
        &self,
        request: Request<FlightReleaseRequest>,
    ) -> Result<Response<FlightReleaseResponse>, Status> {
        let mut compliance_client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(compliance_err_msg)
            .unwrap()
            .compliance_client
            .clone();
        compliance_client.request_flight_release(request).await
    }
}
