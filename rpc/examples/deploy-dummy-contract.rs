use std::str::FromStr;

use anyhow::Context;
use eth_rpc_api::{
    adapters, rlp::*, rpc_methods::*, BlockTag, Bytes, GenericTransaction, TransactionLegacySigned,
    TransactionLegacyUnsigned, H160, H256,
};
use frame::{deps::sp_core::keccak_256, prelude::Encode, traits::Hash};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};

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
        .send_transaction(&client, input.into(), None)
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
        .send_transaction(&client, Bytes::default(), Some(contract_address))
        .await?;
    println!("Contract call tx hash: {hash:?}");
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
