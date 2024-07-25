use eth_rpc::example::Account;
use eth_rpc::EthRpcClient;
use eth_rpc_api::{adapters, Bytes, U256};
use frame::{prelude::Encode, traits::Hash};
use jsonrpsee::http_client::HttpClientBuilder;

static DUMMY_BYTES: &[u8] = include_bytes!("./dummy.wasm");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let account = Account::default();
    println!("Account address: {:?}", account.address());

    let client = HttpClientBuilder::default().build("http://localhost:9090".to_string())?;

    let salt: Vec<u8> = vec![std::env::args()
        .nth(1)
        .unwrap_or("1".to_string())
        .parse()
        .unwrap()];
    println!("Using salt: {salt:?}");

    let data = vec![];
    let input = adapters::CallInput {
        code: DUMMY_BYTES.to_vec(),
        data: data.clone(),
        salt: salt.clone(),
    };

    let input = input.encode();
    let hash = account
        .send_transaction(&client, U256::zero(), input.into(), None)
        .await?;
    println!("Deploy Tx hash: {hash:?}");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let receipt = client.get_transaction_receipt(hash).await;
    println!("Deploy Tx receipt: {receipt:?}");

    let contract_address = primitives::evm_contract_address(
        &primitives::get_account_id(&account.address()),
        &primitives::Hashing::hash(&DUMMY_BYTES),
        &data,
        &salt,
    );
    println!("Contract address: {:?}", contract_address);

    let hash = account
        .send_transaction(
            &client,
            U256::zero(),
            Bytes::default(),
            Some(contract_address),
        )
        .await?;
    println!("Contract call tx hash: {hash:?}");
    Ok(())
}
