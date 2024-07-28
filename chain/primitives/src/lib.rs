//! Core types and traits used in the runtime.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

mod signature;
pub use signature::*;

mod unchecked_extrinsic;
pub use unchecked_extrinsic::*;

use parity_scale_codec::Encode;
use polkadot_sdk::polkadot_sdk_frame::{
    deps::{sp_runtime, sp_runtime::AccountId32},
    derive::Decode,
    primitives::{H160, H256},
    traits::{Hash, TrailingZeroInput},
};

/// Some way of identifying an account on the chain.
pub type AccountId = AccountId32;

/// The address format for describing accounts.
pub type MultiAddress = sp_runtime::MultiAddress<AccountId, AccountIndex>;

/// The type for looking up accounts. We don't expect more than 4 billion of them.
pub type AccountIndex = u32;

/// The hashing algorithm used.
pub type Hashing = sp_runtime::traits::BlakeTwo256;

/// Generate the contract's [`H160`] address from the provided inputs.
/// This function is shared by both the Runtime and the RPC to resolve the contract address.
pub fn evm_contract_address(
    deploying_address: &AccountId,
    code_hash: &H256,
    input_data: &[u8],
    salt: &[u8],
) -> H160 {
    let entropy = (
        b"contract_addr_v1",
        deploying_address,
        code_hash,
        input_data,
        salt,
    )
        .using_encoded(Hashing::hash);
    Decode::decode(&mut TrailingZeroInput::new(entropy.as_ref()))
        .expect("infinite length input; no invalid inputs for type; qed")
}

/// Derive the [`AccountId`] from the provided [`H160`] address.
pub fn get_account_id(evm: &H160) -> AccountId {
    let payload = (b"AccountId32:", evm);
    let bytes = payload.using_encoded(Hashing::hash).0;
    AccountId::new(bytes)
}
