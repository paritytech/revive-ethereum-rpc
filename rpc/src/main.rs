//! Starts the rpc server.
//!
//! TODO make the binary configurable with `clap` so we can customize the src chains, logs and
//! telemetry
use clap::Parser;
use eth_rpc::{client::Client, EthRpcServerImpl, MiscRpcServer, MiscRpcServerImpl};
use eth_rpc_api::rpc_methods::{EthRpcClient, EthRpcServer};
use hyper::Method;
use jsonrpsee::{
    http_client::HttpClientBuilder,
    server::{RpcModule, Server},
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{util::SubscriberInitExt, EnvFilter, FmtSubscriber};

// Parsed command instructions from the command line
#[derive(Parser)]
#[clap(author, about, version)]
struct CliCommand {
    /// The server address to bind to
    #[clap(long, default_value = "127.0.0.1:9090")]
    url: String,

    /// The node url to connect to
    #[clap(long, default_value = "ws://127.0.0.1:9944")]
    node_url: String,
}

/// Initialize tracing
fn init_tracing() {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("eth_rpc=trace"));

    FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish()
        .try_init()
        .expect("failed to initialize tracing");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let CliCommand { url, node_url } = CliCommand::parse();
    init_tracing();

    let client = Client::from_url(&node_url).await.unwrap();
    let mut updates = client.updates.clone();

    let server_addr = run_server(client, &url).await?;
    log::info!("Server started on: {}", server_addr);

    let url = format!("http://{}", server_addr);
    let client = HttpClientBuilder::default().build(url)?;

    let response = client.block_number().await?;
    log::info!("client initialized with block number {:?}", response);

    // keep running server until ctrl-c or client subscription fails
    let _ = updates.wait_for(|_| false).await;
    Ok(())
}

#[cfg(feature = "dev")]
mod dev {
    use futures::{future::BoxFuture, FutureExt};
    use jsonrpsee::{server::middleware::rpc::RpcServiceT, types::Request, MethodResponse};

    /// Dev Logger middleware, that logs the method and params of the request, along with the success of the response.
    #[derive(Clone)]
    pub struct DevLogger<S>(pub S);

    impl<'a, S> RpcServiceT<'a> for DevLogger<S>
    where
        S: RpcServiceT<'a> + Send + Sync + Clone + 'static,
    {
        type Future = BoxFuture<'a, MethodResponse>;

        fn call(&self, req: Request<'a>) -> Self::Future {
            let service = self.0.clone();
            let method = req.method.clone();
            let params = req.params.clone().unwrap_or_default();

            async move {
                let resp = service.call(req).await;
                log::info!("method: {method} params: {params}, success: {}", resp.is_success());
                resp
            }
            .boxed()
        }
    }
}

/// Starts the rpc server and returns the server address.
async fn run_server(client: Client, url: &str) -> anyhow::Result<SocketAddr> {
    let cors = CorsLayer::new()
        .allow_methods([Method::POST])
        .allow_origin(Any)
        .allow_headers([hyper::header::CONTENT_TYPE]);
    let cors_middleware = tower::ServiceBuilder::new().layer(cors);

    let builder = Server::builder().set_http_middleware(cors_middleware);

    #[cfg(feature = "dev")]
    let builder = builder
        .set_rpc_middleware(jsonrpsee::server::RpcServiceBuilder::new().layer_fn(dev::DevLogger));

    let server = builder.build(url.parse::<SocketAddr>()?).await?;
    let addr = server.local_addr()?;

    let eth_api = EthRpcServerImpl::new(client).into_rpc();
    let misc_api = MiscRpcServerImpl.into_rpc();

    let mut module = RpcModule::new(());
    module.merge(eth_api)?;
    module.merge(misc_api)?;

    let handle = server.start(module);
    tokio::spawn(handle.stopped());

    Ok(addr)
}
