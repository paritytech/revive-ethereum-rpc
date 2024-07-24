use eth_rpc::example::Account;
use eth_rpc_api::{rpc_methods::*, BlockTag};
use jsonrpsee::http_client::HttpClientBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let account = Account::default();
    println!("Account address: {:?}", account.address());

    let client = HttpClientBuilder::default().build("http://localhost:9090".to_string())?;

    let block = client
        .get_block_by_number(BlockTag::Latest.into(), false)
        .await?;
    println!("Latest block: {block:#?}");

    let nonce = client
        .get_transaction_count(account.address(), BlockTag::Latest.into())
        .await?;
    println!("Account nonce: {nonce:?}");

    let balance = client
        .get_balance(account.address(), BlockTag::Latest.into())
        .await?;
    println!("Account balance: {balance:?}");

    Ok(())
}
