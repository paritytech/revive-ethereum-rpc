use std::str::FromStr;

use anyhow::Context;
use eth_rpc_api::{
    rlp::*, rpc_methods::*, BlockTag, GenericTransaction, TransactionLegacySigned,
    TransactionLegacyUnsigned, H160,
};
use frame::deps::sp_core::keccak_256;
use hex_literal::hex;
use jsonrpsee::http_client::HttpClientBuilder;
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let account = Account::default();
    println!("Account address: {:?}", account.address());

    let url = "http://localhost:9090".to_string();
    let client = HttpClientBuilder::default().build(url)?;

    let balance = client.get_balance(account.address(), BlockTag::Latest.into()).await?;
    println!("Account balance: {:?}", balance);

    let nonce = client
        .get_transaction_count(account.address(), BlockTag::Latest.into())
        .await
        .with_context(|| "Failed to fetch account nonce")?;

    let gas_price = client.gas_price().await?;

    println!("Account nonce: {nonce:?}");
    let to = Some(H160(hex!("c543bb3eF11d96aCA20b3c906cF2C8Daaff925e4")));
    let value = 10_000_000_000_000_000_000u128.into(); // 10 ETH

    let gas = client
        .estimate_gas(
            GenericTransaction {
                from: Some(account.address()),
                value: Some(value),
                to,
                ..Default::default()
            },
            None,
        )
        .await
        .with_context(|| "Failed to fetch gas estimate")?;

    println!("Gas estimate: {gas:?}");
    let unsigned_tx = TransactionLegacyUnsigned {
        value,
        gas_price,
        gas,
        nonce,
        to,
        chain_id: Some(596.into()),
        ..Default::default()
    };

    let tx = account.sign_transaction(unsigned_tx.clone());
    let bytes = tx.rlp_bytes().to_vec();

    println!("Sending from eth_addr: {:?} with nonce: {nonce}", account.address());

    let hash = client
        .send_raw_transaction(bytes.clone().into())
        .await
        .with_context(|| "transaction failed")?;

    println!("Transaction hash: {hash:?}");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let receipt = client.get_transaction_receipt(hash).await;
    println!("Receipt: {receipt:?}");

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

    fn sign_transaction(&self, tx: TransactionLegacyUnsigned) -> TransactionLegacySigned {
        let rlp_encoded = tx.rlp_bytes();
        let tx_hash = keccak_256(&rlp_encoded);
        let secp = Secp256k1::new();
        let msg = Message::from_digest(tx_hash);
        let sig = secp.sign_ecdsa_recoverable(&msg, &self.sk);
        TransactionLegacySigned::from(tx, sig)
    }
}

//1WSDQWsSewQQVVHkHwt9ezjqifZVQszTC3egnGGkiXHfs1;p
