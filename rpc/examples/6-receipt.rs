#![allow(unused_imports)]
use anyhow::{Context, Result};
use eth_rpc::{
    client::Client,
    subxt_client::{contracts_evm::calls::types::EthInstantiate, Call},
};
use eth_rpc_api::{
    adapters::{CallInput, DryRunInfo},
    TransactionLegacySigned, TransactionLegacyUnsigned, U256,
};
use frame::traits::Hash;
use parity_scale_codec::{Decode, Encode};
use primitives::{Address, MultiSignature};
use secp256k1::ecdsa::{RecoverableSignature, RecoveryId};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let client = Client::from_url("ws://localhost:9944")
        .await
        .context("failed to connect client")?;

    let block = client
        .block_by_number(1)
        .await?
        .ok_or_else(|| anyhow::anyhow!("block not found"))?;

    //0xa3721ca5c7e8f9b688f7312feda01a7403268bd03301fe97f0b0b7556c3bbdf8
    let ext = block.extrinsics().await?;

    let ext = ext
        .find_first::<EthInstantiate>()?
        .ok_or_else(|| anyhow::anyhow!("extrinsic not found"))?;
    let bytes = Vec::from(ext.details.bytes());
    let bytes = bytes.encode();

    let hash = primitives::Hashing::hash(&bytes);

    println!("hash {:?}", hash);
    //Transaction hash: 0xa8a26cfa5ad24e74987eca055b3a4da722402c63e6e25087a421eca37c1c5644
    //hash 0xa8a26cfa5ad24e74987eca055b3a4da722402c63e6e25087a421eca37c1c5644
    Ok(())
}
