/// gRPC test utils for creating gRPC server and client stubs
/// Tonic server and client stubs as per this SO answer:
/// https://stackoverflow.com/questions/69845664/how-to-integration-test-tonic-application
/// and this tonic test example:
/// https://github.com/hyperium/tonic/blob/dee2ab52ff4a2995156a3baf5ea916b479fd1d14/tests/integration_tests/tests/connect_info.rs
use router::location::Location;
use std::future::Future;
use std::sync::Arc;
use svc_storage_client_grpc::client::{
    vehicle_rpc_client::VehicleRpcClient,
    vertiport_rpc_client::VertiportRpcClient,
    vertiport_rpc_server::{VertiportRpc, VertiportRpcServer},
    Id as StorageId, SearchFilter, UpdateVertiport, Vertiport, VertiportData, Vertiports,
};
use tempfile::NamedTempFile;
use tokio::net::{UnixListener, UnixStream};
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::{Channel, Endpoint, Server, Uri};
use tonic::{Request, Response, Status};
use tower::service_fn;

struct VertiportServerStub {}

#[tonic::async_trait]
impl VertiportRpc for VertiportServerStub {
    async fn vertiports(
        &self,
        request: Request<SearchFilter>,
    ) -> Result<Response<Vertiports>, Status> {
        todo!()
    }

    async fn vertiport_by_id(
        &self,
        request: Request<StorageId>,
    ) -> Result<Response<Vertiport>, Status> {
        if request.into_inner().id == "1" {
            Ok(Response::new(Vertiport {
                id: "1".to_string(),
                data: Some(VertiportData {
                    description: "".to_string(),
                    latitude: 0.0,
                    longitude: 0.0,
                    schedule: None,
                }),
            }))
        } else {
            Err(Status::not_found("Not found"))
        }
    }

    async fn insert_vertiport(
        &self,
        request: Request<VertiportData>,
    ) -> Result<Response<Vertiport>, Status> {
        todo!()
    }

    async fn update_vertiport(
        &self,
        request: Request<UpdateVertiport>,
    ) -> Result<Response<Vertiport>, Status> {
        todo!()
    }

    async fn delete_vertiport(&self, request: Request<StorageId>) -> Result<Response<()>, Status> {
        todo!()
    }
}

pub async fn vertiport_server_and_client_stub(
) -> (impl Future<Output = ()>, VertiportRpcClient<Channel>) {
    let socket = NamedTempFile::new().unwrap();
    let socket = Arc::new(socket.into_temp_path());
    std::fs::remove_file(&*socket).unwrap();

    let uds = UnixListener::bind(&*socket).unwrap();
    let stream = UnixListenerStream::new(uds);

    let serve_future = async {
        let result = Server::builder()
            .add_service(VertiportRpcServer::new(VertiportServerStub {}))
            .serve_with_incoming(stream)
            .await;
        // Server must be running fine...
        assert!(result.is_ok());
    };

    let socket = Arc::clone(&socket);
    // Connect to the server over a Unix socket
    // The URL will be ignored.
    let channel = Endpoint::try_from("http://any.url")
        .unwrap()
        .connect_with_connector(service_fn(move |_: Uri| {
            let socket = Arc::clone(&socket);
            async move { UnixStream::connect(&*socket).await }
        }))
        .await
        .unwrap();

    let client = VertiportRpcClient::new(channel);

    (serve_future, client)
}
