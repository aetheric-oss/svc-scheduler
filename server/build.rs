//! build script to generate .rs from .proto

///generates .rs files in src directory
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = "../proto";
    let proto_file = &format!("{}/grpc.proto", proto_dir);

    let server_config = tonic_build::configure()
        .extern_path(".google.protobuf.Timestamp", "::prost_wkt_types::Timestamp")
        .extern_path(
            ".grpc.FlightPlanObject",
            "::svc_storage_client_grpc::prelude::flight_plan::Object",
        )
        .extern_path(
            ".grpc.FlightPlanData",
            "::svc_storage_client_grpc::prelude::flight_plan::Data",
        )
        .extern_path(
            ".grpc.FlightPriority",
            "::svc_storage_client_grpc::prelude::flight_plan::FlightPriority",
        )
        .type_attribute("TaskAction", "#[derive(num_derive::FromPrimitive)]")
        .type_attribute("TaskStatus", "#[derive(num_derive::FromPrimitive)]")
        .type_attribute(
            "TaskMetadata",
            "#[derive(serde::Serialize, serde::Deserialize, Eq)]",
        )
        .type_attribute("ConfirmFlightResponse", "#[derive(Eq)]")
        .type_attribute("CancelFlightResponse", "#[derive(Eq)]")
        .type_attribute("TaskRequest", "#[derive(Eq, Copy)]")
        .type_attribute("TaskResponse", "#[derive(Eq)]")
        .type_attribute("ReadyRequest", "#[derive(Eq, Copy)]")
        .type_attribute("ReadyResponse", "#[derive(Eq, Copy)]")
        .type_attribute("QueryFlightRequest", "#[derive(Eq)]");

    let client_config = server_config.clone();

    client_config
        .client_mod_attribute("grpc", "#[cfg(not(tarpaulin_include))]")
        .build_server(false)
        .out_dir("../client-grpc/src/")
        .compile(&[proto_file], &[proto_dir])?;

    // Build the Server
    server_config
        .build_client(false)
        .compile(&[proto_file], &[proto_dir])?;

    println!("cargo:rerun-if-changed={}", proto_file);

    Ok(())
}
