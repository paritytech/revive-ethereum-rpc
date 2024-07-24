use std::str::FromStr;

use eth_rpc_api::{
    rlp::*, rpc_methods::*, BlockTag, TransactionLegacySigned, TransactionLegacyUnsigned, H160,
};
use frame::deps::sp_core::keccak_256;
use jsonrpsee::http_client::HttpClientBuilder;
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let account = Account::default();
    println!("Account address: {:?}", account.address());

    let url = "http://localhost:9090".to_string();
    let client = HttpClientBuilder::default().build(url)?;

    let nonce = client.get_transaction_count(account.address(), BlockTag::Latest.into()).await?;

    let unsigned_tx = TransactionLegacyUnsigned {
        value: 200_000_000_000u64.into(),
        gas_price: 100_000_000_200u64.into(),
        gas: 100_107u32.into(),
        nonce,
        to: Some(H160::from_str("75e480db528101a381ce68544611c169ad7eb342").unwrap()),
        chain_id: Some(596.into()),
        ..Default::default()
    };
    let tx = account.sign_transaction(unsigned_tx.clone());
    let bytes = tx.rlp_bytes().to_vec();

    println!("Sending from eth_addr: {:?} with nonce: {nonce}", account.address());

    match client.send_raw_transaction(bytes.clone().into()).await {
        Ok(hash) => println!("Transaction hash: {:?}", hash),
        Err(e) => println!("Error: {:?}", e),
    }

    println!(
        "Replay should fail: {}",
        client.send_raw_transaction(bytes.clone().into()).await.is_err()
    );

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
