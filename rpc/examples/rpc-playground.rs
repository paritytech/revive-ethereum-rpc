use std::str::FromStr;

use anyhow::Context;
use eth_rpc_api::{rpc_methods::*, BlockTag, H160};
use frame::deps::sp_core::keccak_256;
use jsonrpsee::http_client::HttpClientBuilder;
use secp256k1::{PublicKey, Secp256k1, SecretKey};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let account = Account::default();
    println!("Account address: {:?}", account.address());

    let url = "http://localhost:9090".to_string();
    let client = HttpClientBuilder::default().build(url)?;

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

struct Account {
    sk: SecretKey,
}

impl Default for Account {
    fn default() -> Self {
        Account {
            sk: SecretKey::from_str(
                "a872f6cbd25a0e04a08b1e21098017a9e6194d101d75e13111f71410c59cd57f",
            )
            .unwrap(),
        }
    }
}

impl Account {
    fn address(&self) -> H160 {
        let pub_key =
            PublicKey::from_secret_key(&Secp256k1::new(), &self.sk).serialize_uncompressed();
        let hash = keccak_256(&pub_key[1..]);
        H160::from_slice(&hash[12..])
    }
}
