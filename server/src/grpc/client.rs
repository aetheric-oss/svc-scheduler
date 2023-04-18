use svc_compliance_client_grpc::client::{
    compliance_rpc_client::ComplianceRpcClient, FlightPlanRequest, FlightPlanResponse,
    FlightReleaseRequest, FlightReleaseResponse,
};

use svc_storage_client_grpc::{
    flight_plan::{
        Data as FlightPlanData, List as FlightPlans, Object as FlightPlan, Response as FPResponse,
        UpdateObject as UpdateFlightPlan,
    },
    itinerary,
    vehicle::List as Vehicles,
    vertipad::List as Vertipads,
    vertiport::{List as Vertiports, Object as Vertiport},
    AdvancedSearchFilter, FlightPlanClient, Id, IdList, ItineraryClient,
    ItineraryFlightPlanLinkClient, VehicleClient, VertipadClient, VertiportClient,
};

use async_trait::async_trait;
use tokio::sync::OnceCell;
use tonic::{transport::Channel, Request, Response, Status};

/// GRPC clients for storage service
/// They have to be cloned before each call as per <https://github.com/hyperium/tonic/issues/285>

pub(crate) static STORAGE_CLIENT_WRAPPER: OnceCell<StorageClientWrapper> = OnceCell::const_new();
pub(crate) static COMPLIANCE_CLIENT_WRAPPER: OnceCell<ComplianceClientWrapper> =
    OnceCell::const_new();

pub(crate) fn get_storage_client_wrapper() -> &'static StorageClientWrapper {
    STORAGE_CLIENT_WRAPPER
        .get()
        .expect("Storage clients not initialized")
}

pub(crate) fn get_compliance_client_wrapper() -> &'static ComplianceClientWrapper {
    COMPLIANCE_CLIENT_WRAPPER
        .get()
        .expect("Compliance client not initialized")
}

fn storage_err_msg() -> Status {
    Status::internal("Storage client not initialized")
}

fn compliance_err_msg() -> Status {
    Status::internal("Compliance client not initialized")
}

/// Initializes grpc clients for storage service
pub async fn init_clients(config: crate::config::Config) {
    //initialize storage client here so it can be used in other methods
    // Storage GRPC Server
    let storage_grpc_port = config.storage_port_grpc;
    let storage_grpc_host = config.storage_host_grpc;
    let storage_full_grpc_addr =
        format!("http://{storage_grpc_host}:{storage_grpc_port}").to_string();

    // Compliance GRPC Server
    let compliance_grpc_port = config.compliance_port_grpc;
    let compliance_grpc_host = config.compliance_host_grpc;
    let compliance_full_grpc_addr =
        format!("http://{compliance_grpc_host}:{compliance_grpc_port}").to_string();

    grpc_info!(
        "Setting up connection to svc-storage clients on {}",
        storage_full_grpc_addr.clone()
    );

    let flight_plan_client_res = FlightPlanClient::connect(storage_full_grpc_addr.clone()).await;
    let vehicle_client_res = VehicleClient::connect(storage_full_grpc_addr.clone()).await;
    let vertiport_client_res = VertiportClient::connect(storage_full_grpc_addr.clone()).await;
    let vertipad_client_res = VertipadClient::connect(storage_full_grpc_addr.clone()).await;
    let itinerary_client_res = ItineraryClient::connect(storage_full_grpc_addr.clone()).await;
    let itinerary_fp_client_res =
        ItineraryFlightPlanLinkClient::connect(storage_full_grpc_addr.clone()).await;

    let compliance_client_res =
        ComplianceRpcClient::connect(compliance_full_grpc_addr.clone()).await;
    if flight_plan_client_res.is_err()
        || vehicle_client_res.is_err()
        || vertiport_client_res.is_err()
        || vertipad_client_res.is_err()
        || itinerary_client_res.is_err()
        || itinerary_fp_client_res.is_err()
    {
        grpc_error!(
            "Failed to connect to storage service at {}. Client errors: {} {} {} {} {} {}",
            storage_full_grpc_addr.clone(),
            flight_plan_client_res.err().unwrap(),
            vehicle_client_res.err().unwrap(),
            vertiport_client_res.err().unwrap(),
            vertipad_client_res.err().unwrap(),
            itinerary_client_res.err().unwrap(),
            itinerary_fp_client_res.err().unwrap()
        );
        panic!();
    } else if compliance_client_res.is_err() {
        grpc_error!(
            "Failed to connect to compliance service at {}. Client errors: {}",
            storage_full_grpc_addr.clone(),
            compliance_client_res.err().unwrap()
        );
        panic!();
    } else {
        let grpc_clients = GRPCClients {
            flight_plan_client: flight_plan_client_res.unwrap(),
            vehicle_client: vehicle_client_res.unwrap(),
            vertiport_client: vertiport_client_res.unwrap(),
            vertipad_client: vertipad_client_res.unwrap(),
            compliance_client: compliance_client_res.unwrap(),
            itinerary_client: itinerary_client_res.unwrap(),
            itinerary_fp_link_client: itinerary_fp_client_res.unwrap(),
        };
        STORAGE_CLIENT_WRAPPER
            .set(StorageClientWrapper {
                grpc_clients: Some(grpc_clients),
            })
            .expect("Failed to set storage client wrapper");
    }
}

#[derive(Debug)]
pub struct GRPCClients {
    pub flight_plan_client: FlightPlanClient<Channel>,
    pub vertiport_client: VertiportClient<Channel>,
    pub vertipad_client: VertipadClient<Channel>,
    pub vehicle_client: VehicleClient<Channel>,
    pub compliance_client: ComplianceRpcClient<Channel>,
    pub itinerary_client: ItineraryClient<Channel>,
    pub itinerary_fp_link_client: ItineraryFlightPlanLinkClient<Channel>,
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
    async fn vehicles(
        &self,
        request: Request<AdvancedSearchFilter>,
    ) -> Result<Response<Vehicles>, Status>;
    async fn vertipads(
        &self,
        request: Request<AdvancedSearchFilter>,
    ) -> Result<Response<Vertipads>, Status>;

    //
    // Itinerary Operations
    //
    async fn itinerary_by_id(
        &self,
        request: Request<Id>,
    ) -> Result<Response<itinerary::Object>, Status>;
    async fn insert_itinerary(
        &self,
        request: Request<itinerary::Data>,
    ) -> Result<Response<itinerary::Response>, Status>;
    async fn update_itinerary(
        &self,
        request: Request<itinerary::UpdateObject>,
    ) -> Result<Response<itinerary::Response>, Status>;

    //
    // Itinerary/Flight Plan Link Operations
    //
    async fn link_flight_plan(
        &self,
        request: Request<itinerary::ItineraryFlightPlans>,
    ) -> Result<Response<()>, Status>;

    async fn get_itinerary_flight_plan_ids(
        &self,
        request: Request<Id>,
    ) -> Result<Response<IdList>, Status>;
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

    async fn vehicles(
        &self,
        request: Request<AdvancedSearchFilter>,
    ) -> Result<Response<Vehicles>, Status> {
        let mut vehicle_client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(storage_err_msg)
            .unwrap()
            .vehicle_client
            .clone();
        vehicle_client.search(request).await
    }

    async fn vertipads(
        &self,
        request: Request<AdvancedSearchFilter>,
    ) -> Result<Response<Vertipads>, Status> {
        let mut vertipad_client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(storage_err_msg)
            .unwrap()
            .vertipad_client
            .clone();
        vertipad_client.search(request).await
    }

    async fn itinerary_by_id(
        &self,
        request: Request<Id>,
    ) -> Result<Response<itinerary::Object>, Status> {
        let mut client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(storage_err_msg)
            .unwrap()
            .itinerary_client
            .clone();
        client.get_by_id(request).await
    }

    async fn insert_itinerary(
        &self,
        request: Request<itinerary::Data>,
    ) -> Result<Response<itinerary::Response>, Status> {
        let mut client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(storage_err_msg)
            .unwrap()
            .itinerary_client
            .clone();
        client.insert(request).await
    }

    async fn update_itinerary(
        &self,
        request: Request<itinerary::UpdateObject>,
    ) -> Result<Response<itinerary::Response>, Status> {
        let mut client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(storage_err_msg)
            .unwrap()
            .itinerary_client
            .clone();
        client.update(request).await
    }

    async fn link_flight_plan(
        &self,
        request: Request<itinerary::ItineraryFlightPlans>,
    ) -> Result<Response<()>, Status> {
        let mut client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(storage_err_msg)
            .unwrap()
            .itinerary_fp_link_client
            .clone();
        client.link(request).await
    }

    async fn get_itinerary_flight_plan_ids(
        &self,
        request: Request<Id>,
    ) -> Result<Response<IdList>, Status> {
        let mut client = self
            .grpc_clients
            .as_ref()
            .ok_or_else(storage_err_msg)
            .unwrap()
            .itinerary_fp_link_client
            .clone();
        client.get_linked_ids(request).await
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
