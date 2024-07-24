use anyhow::{Context, Result};
use eth_rpc::{
    client::Client,
    subxt_client::{self, transaction_payment::events::TransactionFeePaid},
};
use eth_rpc_api::rpc_methods::EthRpcClient;
use jsonrpsee::http_client::HttpClientBuilder;
use primitives::MultiSignature;
use subxt::config::DefaultExtrinsicParamsBuilder;
use subxt_signer::sr25519::dev::{alice, bob};

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::from_url("ws://localhost:9944")
        .await
        .context("failed to connect client")?;

    let alice = alice();
    let bob = bob();

    let call = subxt_client::tx().balances().transfer_keep_alive(bob.public_key().into(), 50);

    let tx_params = DefaultExtrinsicParamsBuilder::default().tip(2).build();

    let tx = client
        .tx()
        .create_partial_signed(&call, &alice.public_key().into(), tx_params)
        .await
        .context("failed to create partial signed tx")?;

    let signature = alice.sign(&tx.signer_payload());
    let signature = MultiSignature::Sr25519(signature.0.into());
    let tx = tx.sign_with_address_and_signature(&alice.public_key().to_address(), &signature);

    println!("{:<20} {:?}", "tx hash", tx.hash());
    println!("bytes : {:?} len : {}", tx.encoded()[0..12].to_vec(), tx.encoded().len());
    let progress = tx.submit_and_watch().await.context("failed to submit tx")?;
    println!("{:<20} {:?}", "submit hash", progress.extrinsic_hash());

    let tx = progress.wait_for_finalized().await?;
    let events = tx.fetch_events().await?;
    let fee_event = events
        .find_first::<TransactionFeePaid>()
        .context("failed to get TransactionFeePaid event")?
        .ok_or_else(|| anyhow::anyhow!("TransactionFeePaid event not found"))?;

    println!("fee paid: {:?}", fee_event);
    println!("in block tx hash {:?}", tx.extrinsic_hash());

    let url = "http://localhost:9090".to_string();
    let client = HttpClientBuilder::default().build(url)?;
    let block = client.block_number().await?;
    println!("block number: {:?}", block);

    Ok(())
}
