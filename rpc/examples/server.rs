use eth_rpc::{client::Client, EthRpcServerImpl, MiscRpcClient, MiscRpcServer, MiscRpcServerImpl};
use eth_rpc_api::rpc_methods::*;
use jsonrpsee::{http_client::HttpClientBuilder, server::Server, RpcModule};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server_addr = run_server().await?;
    println!("Server started on: {}", server_addr);

    let url = format!("http://{}", server_addr);
    let client = HttpClientBuilder::default().build(url)?;

    let response = client.block_number().await?;
    println!("{:?}", response);

    client.healthcheck().await?;

    // keep running server until ctrl-c
    let () = futures::future::pending().await;
    Ok(())
}

async fn run_server() -> anyhow::Result<SocketAddr> {
    let server = Server::builder().build("127.0.0.1:8080".parse::<SocketAddr>()?).await?;

    let addr = server.local_addr()?;

    let client = Client::from_url("ws://localhost:9944").await.unwrap();
    let eth_api = EthRpcServerImpl::new(client).into_rpc();
    let misc_api = MiscRpcServerImpl.into_rpc();

    let mut rpc_api = RpcModule::new(());
    rpc_api.merge(eth_api)?;
    rpc_api.merge(misc_api)?;

    let handle = server.start(rpc_api);

    tokio::spawn(handle.stopped());
    Ok(addr)
}
