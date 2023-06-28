//! gRPC server implementation

use log::info;
use svc_scheduler::*;

#[tokio::main]
#[cfg(not(tarpaulin_include))]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("(svc-scheduler) server startup.");

    // Will use default config settings if no environment vars are found.
    let config = Config::try_from_env().unwrap_or_default();

    init_logger(&config);

    // Spawn the loop for the router re-initialization
    // (Dirty hack)
    // TODO(R3): Refactor to respond to grpc trigger or
    //  move routing to SQL Graph database
    tokio::spawn(async move {
        // Every 10 seconds
        let duration = std::time::Duration::new(10, 0);

        loop {
            grpc::queries::init_router().await;

            std::thread::sleep(duration);
        }
    });

    // Spawn the GRPC server for this service
    tokio::spawn(grpc::server::grpc_server(config, None)).await?;

    // Make sure all log message are written/ displayed before shutdown
    log::logger().flush();

    info!("(svc-scheduler) server shutdown.");

    Ok(())
}
