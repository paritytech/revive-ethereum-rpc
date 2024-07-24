use eth_rpc_api::{rlp::*, *};
use frame::deps::sp_core::hashing::keccak_256;
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
use std::str::FromStr;

fn main() {
    let account = Account::default();

    let unsigned_tx = TransactionLegacyUnsigned {
        value: 200_000_000_000_000_000_000u128.into(),
        gas_price: 100_000_000_200u64.into(),
        gas: 100_107u32.into(),
        nonce: 3.into(),
        to: Some(H160::from_str("75e480db528101a381ce68544611c169ad7eb342").unwrap()),
        chain_id: Some(596.into()),
        ..Default::default()
    };

    let tx = account.sign_transaction(unsigned_tx.clone());
    let recovered_address = tx.recover_eth_address().unwrap();
    let signature = hex::encode(tx.raw_signature().unwrap());

    println!("Account address:   {:?}", account.address());
    println!("recovered address: {:?}", recovered_address);
    println!("signature:         0x{}", signature);

    assert_eq!(account.address(), recovered_address);
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
