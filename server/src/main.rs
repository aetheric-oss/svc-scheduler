//! gRPC server implementation

mod config;
mod grpc;
mod queries;
mod router;

use dotenv::dotenv;
use log::{error, info};

///Main entry point: starts gRPC Server on specified address and port
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    info!("(svc-scheduler) server startup.");

    //initialize dotenv library which reads .env file
    dotenv().ok();

    // Will use default config settings if no environment vars are found.
    let config = config::Config::from_env().unwrap_or_default();

    // Initialize logger
    let log_cfg: &str = config.log_config.as_str();
    if let Err(e) = log4rs::init_file(log_cfg, Default::default()) {
        error!("(logger) could not parse {}. {}", log_cfg, e);
        panic!();
    }

    // Initialize storage client here so it can be used in other methods
    grpc::client::init_clients(config.clone()).await;

    // Spawn the loop for the router re-initialization
    // (Dirty hack)
    // TODO(R3): Refactor to respond to grpc trigger or
    //  move routing to SQL Graph database
    tokio::spawn(async move {
        use grpc::client::get_storage_client_wrapper;

        // Every 10 seconds
        let duration = std::time::Duration::new(10, 0);

        loop {
            queries::init_router(get_storage_client_wrapper()).await;

            std::thread::sleep(duration);
        }
    });

    // Spawn the GRPC server for this service
    let _ = tokio::spawn(grpc::server::server(config)).await;

    info!("(svc-scheduler) server shutdown.");
    Ok(())
}
