use std::str::FromStr;

use anyhow::Context;
use eth_rpc_api::{
    rlp::*, rpc_methods::*, BlockTag, Bytes, GenericTransaction, TransactionLegacySigned,
    TransactionLegacyUnsigned, H160, H256, U256,
};
use frame::deps::sp_core::keccak_256;
use hex_literal::hex;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let account = Account::default();
    println!("Account address: {:?}", account.address());

    let client = HttpClientBuilder::default().build("http://localhost:9090".to_string())?;

    let balance = client
        .get_balance(account.address(), BlockTag::Latest.into())
        .await?;
    println!("Account balance: {:?}", balance);

    let to = Some(H160(hex!("c543bb3eF11d96aCA20b3c906cF2C8Daaff925e4")));
    let value = 10_000_000_000_000_000_000u128.into(); // 10 ETH

    let hash = account
        .send_transaction(&client, value, Bytes::default(), to)
        .await?;
    println!("Transaction hash: {hash:?}");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let receipt = client.get_transaction_receipt(hash).await;
    println!("Receipt: {receipt:?}");

    let balance = client
        .get_balance(account.address(), BlockTag::Latest.into())
        .await?;
    println!("Account balance: {:?}", balance);

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

    async fn send_transaction(
        &self,
        client: &HttpClient,
        value: U256,
        input: Bytes,
        to: Option<H160>,
    ) -> anyhow::Result<H256> {
        let from = self.address();

        let chain_id = Some(client.chain_id().await?);

        let gas_price = client.gas_price().await?;
        let nonce = client
            .get_transaction_count(from, BlockTag::Latest.into())
            .await
            .with_context(|| "Failed to fetch account nonce")?;

        let gas = client
            .estimate_gas(
                GenericTransaction {
                    from: Some(from),
                    input: Some(input.clone()),
                    gas_price: Some(gas_price),
                    value: Some(value),
                    to,
                    ..Default::default()
                },
                None,
            )
            .await
            .with_context(|| "Failed to fetch gas estimate")?;

        let unsigned_tx = TransactionLegacyUnsigned {
            gas,
            nonce,
            to,
            input,
            gas_price,
            chain_id,
            ..Default::default()
        };

        let tx = self.sign_transaction(unsigned_tx.clone());
        let bytes = tx.rlp_bytes().to_vec();

        let hash = client
            .send_raw_transaction(bytes.clone().into())
            .await
            .with_context(|| "transaction failed")?;

        Ok(hash)
    }
}
