//! gRPC client helpers implementation
use lib_common::grpc::{Client, GrpcClient};
use svc_compliance_client_grpc::client::rpc_service_client::RpcServiceClient as ComplianceClient;
use svc_storage_client_grpc::Clients;
use tokio::sync::OnceCell;
use tonic::transport::Channel;

pub(crate) static CLIENTS: OnceCell<GrpcClients> = OnceCell::const_new();

/// Returns CLIENTS, a GrpcClients object with default values.
/// Uses host and port configurations using a Config object generated from
/// environment variables.
/// Initializes CLIENTS if it hasn't been initialized yet.
pub async fn get_clients() -> &'static GrpcClients {
    CLIENTS
        .get_or_init(|| async move {
            let config = crate::Config::try_from_env().unwrap_or_default();
            GrpcClients::default(config)
        })
        .await
}

/// Struct to hold all gRPC client connections
#[derive(Clone, Debug)]
pub struct GrpcClients {
    /// All clients enabled from the svc_storage_grpc_client module
    pub storage: Clients,
    /// A GrpcClient provided by the svc_compliance_grpc_client module
    pub compliance: GrpcClient<ComplianceClient<Channel>>,
}

impl GrpcClients {
    /// Create new GrpcClients with defaults
    pub fn default(config: crate::config::Config) -> Self {
        let storage_clients = Clients::new(config.storage_host_grpc, config.storage_port_grpc);

        GrpcClients {
            storage: storage_clients,
            compliance: GrpcClient::<ComplianceClient<Channel>>::new_client(
                &config.compliance_host_grpc,
                config.compliance_port_grpc,
                "compliance",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use svc_compliance_client_grpc::Client as ComplianceServiceClient;

    use super::*;

    #[tokio::test]
    async fn test_grpc_clients_default() {
        let clients = get_clients().await;

        let vehicle = &clients.storage.vehicle;
        println!("{:?}", vehicle);
        assert_eq!(vehicle.get_name(), "vehicle");

        let vertipad = &clients.storage.vertipad;
        println!("{:?}", vertipad);
        assert_eq!(vertipad.get_name(), "vertipad");

        let vertiport = &clients.storage.vertiport;
        println!("{:?}", vertiport);
        assert_eq!(vertiport.get_name(), "vertiport");

        let itinerary = &clients.storage.itinerary;
        println!("{:?}", itinerary);
        assert_eq!(itinerary.get_name(), "itinerary");

        let itinerary_flight_plan = &clients.storage.itinerary_flight_plan_link;
        println!("{:?}", itinerary_flight_plan);
        assert_eq!(
            itinerary_flight_plan.get_name(),
            "itinerary_flight_plan_link"
        );

        let flight_plan = &clients.storage.flight_plan;
        println!("{:?}", flight_plan);
        assert_eq!(flight_plan.get_name(), "flight_plan");

        let compliance = &clients.compliance;
        println!("{:?}", compliance);
        assert_eq!(compliance.get_name(), "compliance");
    }
}
