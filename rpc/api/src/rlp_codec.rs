//! RLP encoding and decoding for Ethereum transactions.

use ethereum_types::H160;
use frame::deps::{sp_core::keccak_256, sp_io::crypto::secp256k1_ecdsa_recover};
use rlp::{Decodable, Encodable};

use super::*;

impl Decodable for TransactionSigned {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if rlp.is_null() {
            return Err(rlp::DecoderError::RlpInvalidLength);
        }
        if rlp.is_list() {

            //Ok(TransactionLegacySigned::decode(rlp)?)
        }

        match rlp.as_raw()[0] {
            0x01 => {
                //Ok(TransactionLegacySigned::decode(rlp)?)
                todo!()
            },
            0x02 => {
                //Ok(TransactionLegacySigned::decode(rlp)?)
                todo!()
            },
            _ => Err(rlp::DecoderError::Custom("Invalid transaction type")),
        }
    }
}

impl Encodable for TransactionLegacyUnsigned {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        if let Some(chain_id) = self.chain_id {
            s.begin_list(9);
            s.append(&self.nonce);
            s.append(&self.gas_price);
            s.append(&self.gas);
            match self.to {
                Some(ref to) => s.append(to),
                None => s.append_empty_data(),
            };
            s.append(&self.value);
            s.append(&self.input.0);
            s.append(&chain_id);
            s.append(&0_u8);
            s.append(&0_u8);
        } else {
            s.begin_list(6);
            s.append(&self.nonce);
            s.append(&self.gas_price);
            s.append(&self.gas);
            match self.to {
                Some(ref to) => s.append(to),
                None => s.append_empty_data(),
            };
            s.append(&self.value);
            s.append(&self.input.0);
        }
    }
}

impl Decodable for TransactionLegacyUnsigned {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        Ok(TransactionLegacyUnsigned {
            nonce: rlp.val_at(0)?,
            gas_price: rlp.val_at(1)?,
            gas: rlp.val_at(2)?,
            to: {
                let to = rlp.at(3)?;
                if to.is_empty() {
                    None
                } else {
                    Some(to.as_val()?)
                }
            },
            value: rlp.val_at(4)?,
            input: Bytes(rlp.val_at(5)?),
            chain_id: {
                if let Ok(chain_id) = rlp.val_at(6) {
                    Some(chain_id)
                } else {
                    None
                }
            },
            ..Default::default()
        })
    }
}

impl Encodable for TransactionLegacySigned {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(9);
        s.append(&self.transaction_legacy_unsigned.nonce);
        s.append(&self.transaction_legacy_unsigned.gas_price);
        s.append(&self.transaction_legacy_unsigned.gas);
        match self.transaction_legacy_unsigned.to {
            Some(ref to) => s.append(to),
            None => s.append_empty_data(),
        };
        s.append(&self.transaction_legacy_unsigned.value);
        s.append(&self.transaction_legacy_unsigned.input.0);

        s.append(&self.v);
        s.append(&self.r);
        s.append(&self.s);
    }
}

impl Decodable for TransactionLegacySigned {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let v: U256 = rlp.val_at(6)?;
        let extract_chain_id = |v: u64| {
            if v >= 35 {
                Some((v - 35) / 2)
            } else {
                None
            }
        };

        Ok(TransactionLegacySigned {
            transaction_legacy_unsigned: {
                TransactionLegacyUnsigned {
                    nonce: rlp.val_at(0)?,
                    gas_price: rlp.val_at(1)?,
                    gas: rlp.val_at(2)?,
                    to: {
                        let to = rlp.at(3)?;
                        if to.is_empty() {
                            None
                        } else {
                            Some(to.as_val()?)
                        }
                    },
                    value: rlp.val_at(4)?,
                    input: Bytes(rlp.val_at(5)?),
                    chain_id: extract_chain_id(v.as_u64()).map(|v| v.into()),
                    r#type: Type0 {},
                }
            },
            v,
            r: rlp.val_at(7)?,
            s: rlp.val_at(8)?,
        })
    }
}

pub trait SignerRecovery {
    type Signature;
    type Signer;
    fn recover_signer(&self, signature: &Self::Signature) -> Option<Self::Signer>;
}

impl SignerRecovery for TransactionLegacyUnsigned {
    type Signature = [u8; 65];
    type Signer = H160;
    fn recover_signer(&self, sig: &Self::Signature) -> Option<Self::Signer> {
        let msg = keccak_256(&rlp::encode(self));
        let pub_key = secp256k1_ecdsa_recover(sig, &msg).ok()?;
        let pub_key_hash = keccak_256(&pub_key);
        Some(H160::from_slice(&pub_key_hash[12..]))
    }
}

impl Encodable for AccessListEntry {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2);
        s.append(&self.address);
        s.append_list(&self.storage_keys);
    }
}

impl Encodable for Transaction2930Unsigned {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(8);
        s.append(&self.chain_id);
        s.append(&self.nonce);
        s.append(&self.gas_price);
        s.append(&self.gas);
        match self.to {
            Some(ref to) => s.append(to),
            None => s.append_empty_data(),
        };
        s.append(&self.value);
        s.append(&self.input.0);
        s.append_list(&self.access_list);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use core::str::FromStr;
    use hex_literal::hex;
    use pretty_assertions::assert_eq;
    use secp256k1::{Message, Secp256k1, SecretKey};

    // see https://eips.ethereum.org/EIPS/eip-155
    #[test]
    fn eip_155_encoding_works() {
        let chain_id = 1u32;
        let tx = TransactionLegacyUnsigned {
            nonce: 9.into(),
            gas_price: (20 * 10u128.pow(9)).into(),
            gas: 21000u32.into(),
            to: Some(H160::from_str("0x3535353535353535353535353535353535353535").unwrap()),
            value: 10u128.pow(18).into(),
            chain_id: Some(chain_id.into()),
            ..Default::default()
        };

        let bytes = tx.rlp_bytes();
        assert_eq!(
            bytes.to_vec(),
            hex!("ec098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a764000080018080")
        );

        let tx_hash = keccak_256(&bytes);
        assert_eq!(
            tx_hash,
            hex!("daf5a779ae972f972197303d7b574746c7ef83eadac0f2791ad23db92e4c8e53")
        );

        let sk =
            SecretKey::from_str("4646464646464646464646464646464646464646464646464646464646464646")
                .unwrap();

        let msg = Message::from_digest(tx_hash);
        let secp = Secp256k1::new();
        let sig = secp.sign_ecdsa_recoverable(&msg, &sk);
        let signed_tx = TransactionLegacySigned::from(tx, sig);

        assert_eq!(signed_tx.v, U256::from(37));
        assert_eq!(
            signed_tx.r,
            U256::from_dec_str(
                "18515461264373351373200002665853028612451056578545711640558177340181847433846"
            )
            .unwrap()
        );
        assert_eq!(
            signed_tx.s,
            U256::from_dec_str(
                "46948507304638947509940763649030358759909902576025900602547168820602576006531"
            )
            .unwrap()
        );

        let bytes = signed_tx.rlp_bytes();

        assert_eq!(
            bytes.to_vec(),
            hex!("f86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83")
        );
    }

    #[test]
    fn encode_decode_legacy_transaction() {
        let tx = TransactionLegacyUnsigned {
            chain_id: Some(596.into()),
            gas: U256::from(21000),
            nonce: U256::from(1),
            gas_price: U256::from("0x640000006a"),
            to: Some(H160::from_str("0x1111111111222222222233333333334444444444").unwrap()),
            value: U256::from(123123),
            input: Bytes(vec![]),
            r#type: Type0,
        };

        let rlp_bytes = rlp::encode(&tx);
        let decoded = rlp::decode::<TransactionLegacyUnsigned>(&rlp_bytes).unwrap();
        dbg!(&decoded);
    }
}
