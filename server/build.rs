//! build script to generate .rs from .proto

///generates .rs files in src directory
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server_config = tonic_build::configure()
        .type_attribute("QueryFlightPlan", "#[derive(Eq)]")
        .type_attribute("ConfirmFlightResponse", "#[derive(Eq)]")
        .type_attribute("CancelFlightResponse", "#[derive(Eq)]")
        .type_attribute("QueryFlightResponse", "#[derive(Eq)]")
        .type_attribute("Id", "#[derive(Eq)]")
        .type_attribute("ReadyRequest", "#[derive(Eq, Copy)]")
        .type_attribute("ReadyResponse", "#[derive(Eq, Copy)]")
        .type_attribute("QueryFlightRequest", "#[derive(Eq)]");
    let client_config = server_config.clone();

    server_config
        .build_client(false)
        .out_dir("src/")
        .compile(&["../proto/svc-scheduler-grpc.proto"], &["../proto"])?;

    client_config
        .build_server(false)
        .out_dir("../client/src")
        .compile(&["../proto/svc-scheduler-grpc.proto"], &["../proto"])?;

    Ok(())
}
