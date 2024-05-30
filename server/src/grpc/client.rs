//! gRPC client helpers implementation
use svc_gis_client_grpc::prelude::Client;
use svc_gis_client_grpc::prelude::GisClient;
use svc_storage_client_grpc::prelude::Clients;
use tokio::sync::OnceCell;

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
    /// A GrpcClient provided by the svc_gis_grpc_client module
    pub gis: GisClient,
}

impl GrpcClients {
    /// Create new GrpcClients with defaults
    pub fn default(config: crate::config::Config) -> Self {
        let storage_clients = Clients::new(config.storage_host_grpc, config.storage_port_grpc);

        GrpcClients {
            storage: storage_clients,
            gis: GisClient::new_client(&config.gis_host_grpc, config.gis_port_grpc, "gis"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use svc_gis_client_grpc::prelude::Client as GisClient;
    use svc_storage_client_grpc::prelude::Client as StorageClient;

    #[tokio::test]
    async fn test_grpc_clients_default() {
        lib_common::logger::get_log_handle().await;
        ut_info!("Start.");

        let clients = get_clients().await;

        let vehicle = &clients.storage.vehicle;
        ut_debug!("vehicle: {:?}", vehicle);
        assert_eq!(vehicle.get_name(), "vehicle");

        let vertipad = &clients.storage.vertipad;
        ut_debug!("vertipad: {:?}", vertipad);
        assert_eq!(vertipad.get_name(), "vertipad");

        let vertiport = &clients.storage.vertiport;
        ut_debug!("vertiport: {:?}", vertiport);
        assert_eq!(vertiport.get_name(), "vertiport");

        let itinerary = &clients.storage.itinerary;
        ut_debug!("itinerary: {:?}", itinerary);
        assert_eq!(itinerary.get_name(), "itinerary");

        let itinerary_flight_plan = &clients.storage.itinerary_flight_plan_link;
        ut_debug!("itinerary_flight_plan: {:?}", itinerary_flight_plan);
        assert_eq!(
            itinerary_flight_plan.get_name(),
            "itinerary_flight_plan_link"
        );

        let flight_plan = &clients.storage.flight_plan;
        ut_debug!("flight_plan: {:?}", flight_plan);
        assert_eq!(flight_plan.get_name(), "flight_plan");

        let gis = &clients.gis;
        ut_debug!("gis: {:?}", gis);
        assert_eq!(gis.get_name(), "gis");

        ut_info!("Success.");
    }
}
