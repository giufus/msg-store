use service::StorageService;
use storage::storage_server::StorageServer;
use tonic::transport::Server;

pub mod storage {
    tonic::include_proto!("messages");
}

mod service;


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:8080".parse()?;
    let storage_service = StorageService::default();

    Server::builder()
        .add_service(StorageServer::new(storage_service))
        .serve(addr)
        .await?;

    Ok(())
}
