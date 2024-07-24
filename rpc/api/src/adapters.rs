use crate::{
    Bytes, GenericTransaction, ReceiptInfo, TransactionInfo, TransactionLegacySigned,
    TransactionLegacyUnsigned, U256,
};
use ethereum_types::H160;
use frame::{
    deps::{sp_core::blake2_256, sp_runtime::AccountId32},
    prelude::*,
};

/// Convert an EVM address to a Substrate account ID.
pub fn get_account_id(evm: &H160) -> AccountId32 {
    let payload = (b"AccountId32:", evm);
    AccountId32::new(payload.using_encoded(blake2_256))
}

impl TransactionLegacyUnsigned {
    /// Convert a legacy transaction to a [`GenericTransaction`].
    pub fn as_generic(self, from: H160) -> GenericTransaction {
        GenericTransaction {
            from: Some(from),
            chain_id: self.chain_id,
            gas: Some(self.gas),
            input: Some(self.input),
            nonce: Some(self.nonce),
            to: self.to,
            r#type: Some(self.r#type.as_byte()),
            value: Some(self.value),
            ..Default::default()
        }
    }

    /// Build a transaction from an instantiate call.
    pub fn from_instantiate(
        input: CallInput,
        value: U256,
        gas_price: U256,
        gas: U256,
        nonce: U256,
        chain_id: U256,
    ) -> Self {
        Self {
            input: Bytes(input.encode()),
            value,
            gas_price,
            gas,
            nonce,
            chain_id: Some(chain_id),
            ..Default::default()
        }
    }

    /// Build a transaction from a call.
    pub fn from_call(
        to: H160,
        input: Vec<u8>,
        value: U256,
        gas_price: U256,
        gas: U256,
        nonce: U256,
        chain_id: U256,
    ) -> Self {
        Self {
            to: Some(to),
            input: Bytes(input),
            value,
            gas_price,
            gas,
            nonce,
            chain_id: Some(chain_id),
            ..Default::default()
        }
    }
}

/// The input for a call.
///
/// It can be encoded as a [`Vec<u8>`] and passed as the **input** field of a transaction.
#[derive(Clone, Encode, Decode, Debug)]
pub struct CallInput {
    pub code: Vec<u8>,
    pub data: Vec<u8>,
    pub salt: Vec<u8>,
}

/// The output information of a dry run.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct DryRunInfo<Balance> {
    pub gas_limit: Weight,
    pub storage_deposit_limit: Option<Balance>,
    pub return_data: Vec<u8>,
}

// TODO: store the transaction_signed in the cache so that we can populate `transaction_signed`
impl From<ReceiptInfo> for TransactionInfo {
    fn from(receipt: ReceiptInfo) -> Self {
        Self {
            block_hash: receipt.block_hash,
            block_number: receipt.block_number,
            from: receipt.from,
            hash: receipt.transaction_hash,
            transaction_index: receipt.transaction_index,
            transaction_signed: crate::TransactionSigned::TransactionLegacySigned(
                TransactionLegacySigned::default(),
            ),
        }
    }
}
