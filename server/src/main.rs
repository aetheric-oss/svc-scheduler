//! gRPC server implementation

use log::info;
use svc_scheduler::*;

#[tokio::main]
#[cfg(not(tarpaulin_include))]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("(main) server startup.");

    // Will use default config settings if no environment vars are found.
    let config = Config::try_from_env().unwrap_or_default();

    init_logger(&config);

    // Spawn the GRPC server for this service
    tokio::spawn(grpc::server::grpc_server(config, None)).await?;

    // Make sure all log message are written/ displayed before shutdown
    log::logger().flush();

    info!("(main) server shutdown.");

    Ok(())
}
