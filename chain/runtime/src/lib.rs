//! A minimal runtime that includes [`pallet-contracts`] and [`pallet-contracts-evm`].

#![cfg_attr(not(feature = "std"), no_std)]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use eth_rpc_api::{adapters::*, U256};

use eth_rpc_api::{GenericTransaction, TransactionLegacyUnsigned, TransactionUnsigned};
use frame::{
    deps::frame_support::{
        genesis_builder_helper::{build_state, get_preset},
        runtime,
        weights::{FixedFee, IdentityFee, WeightToFee},
    },
    log,
    prelude::*,
    primitives::{BlakeTwo256, H160, H256},
    runtime::{
        apis::{
            self, impl_runtime_apis, ApplyExtrinsicResult, CheckInherentsResult,
            ExtrinsicInclusionMode, OpaqueMetadata,
        },
        prelude::*,
        types_common::BlockNumber,
    },
};
use frame::{deps::sp_runtime::transaction_validity::InvalidTransaction, traits::Convert};
use frame_system::limits::BlockWeights;
use interface::*;
use pallet_contracts::AddressGenerator;
use pallet_contracts_evm::{EVMAddressMapping, EVM_DECIMALS};
use pallet_contracts_evm_primitives::{evm_contract_address, UncheckedExtrinsic};
use pallet_transaction_payment::{FeeDetails, RuntimeDispatchInfo};
use sp_runtime::generic;

/// The runtime version.
#[runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("minimal-runtime"),
    impl_name: create_runtime_str!("minimal-runtime"),
    authoring_version: 1,
    spec_version: 0,
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
    state_version: 1,
};

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

/// The signed extensions that are added to the runtime.
type SignedExtra = (
    // Checks that the sender is not the zero address.
    frame_system::CheckNonZeroSender<Runtime>,
    // Checks that the runtime version is correct.
    frame_system::CheckSpecVersion<Runtime>,
    // Checks that the transaction version is correct.
    frame_system::CheckTxVersion<Runtime>,
    // Checks that the genesis hash is correct.
    frame_system::CheckGenesis<Runtime>,
    // Checks that the era is valid.
    frame_system::CheckEra<Runtime>,
    // Checks that the nonce is valid.
    frame_system::CheckNonce<Runtime>,
    // Checks that the weight is valid.
    frame_system::CheckWeight<Runtime>,
    // Ensures that the sender has enough funds to pay for the transaction
    // and deducts the fee from the sender's account.
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);

// Composes the runtime by adding all the used pallets and deriving necessary types.
#[runtime]
mod runtime {
    /// The main runtime type.
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask
    )]
    pub struct Runtime;

    /// Mandatory system pallet that should always be included in a FRAME runtime.
    #[runtime::pallet_index(0)]
    pub type System = frame_system;

    /// Provides a way for consensus systems to set and check the onchain time.
    #[runtime::pallet_index(1)]
    pub type Timestamp = pallet_timestamp;

    /// Provides the ability to keep track of balances.
    #[runtime::pallet_index(2)]
    pub type Balances = pallet_balances;

    /// Provides a way to execute privileged functions.
    #[runtime::pallet_index(3)]
    pub type Sudo = pallet_sudo;

    /// Provides the ability to charge for extrinsic execution.
    #[runtime::pallet_index(4)]
    pub type TransactionPayment = pallet_transaction_payment;

    /// Provides the ability to deploy and interact with smart contracts.
    #[runtime::pallet_index(5)]
    pub type Contracts = pallet_contracts;

    /// Provides the ability to deploy and interact with EVM wallets.
    #[runtime::pallet_index(6)]
    pub type ContractsEvm = pallet_contracts_evm;
}

parameter_types! {
  pub const Version: RuntimeVersion = VERSION;
}

/// Implements the types required for the system pallet.
#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig)]
impl frame_system::Config for Runtime {
    type Block = Block;
    type Version = Version;
    type Lookup = ContractsEvm;
    // Use the account data from the balances pallet
    type AccountData = pallet_balances::AccountData<<Runtime as pallet_balances::Config>::Balance>;
}

// Implements the types required for the balances pallet.
#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Runtime {
    type AccountStore = System;
    type Balance = u128;
    type ExistentialDeposit = ConstU128<1_000>;
}

// Implements the types required for the sudo pallet.
#[derive_impl(pallet_sudo::config_preludes::TestDefaultConfig)]
impl pallet_sudo::Config for Runtime {}

// Implements the types required for the sudo pallet.
#[derive_impl(pallet_timestamp::config_preludes::TestDefaultConfig)]
impl pallet_timestamp::Config for Runtime {}

// Implements the types required for the transaction payment pallet.
#[derive_impl(pallet_transaction_payment::config_preludes::TestDefaultConfig)]
impl pallet_transaction_payment::Config for Runtime {
    type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
    type WeightToFee = IdentityFee<<Self as pallet_balances::Config>::Balance>;
    type LengthToFee = FixedFee<100, <Self as pallet_balances::Config>::Balance>;
}

parameter_types! {
  pub ContractSchedule: pallet_contracts::Schedule<Runtime> = Default::default();
  pub const DepositPerItem: Balance = 10_000;
  pub const DepositPerByte: Balance = 10_000;
}

/// A struct implementing the [`AddressGenerator`] trait, which derives the contract's address
/// using the shared [`evm_contract_address`] function. This function is utilized by both the runtime
/// and the proxy to ensure consistent contract address derivation.
pub struct EvmAddressGenerator;
impl AddressGenerator<Runtime> for EvmAddressGenerator {
    fn contract_address(
        deploying_address: &AccountId,
        code_hash: &H256,
        input_data: &[u8],
        salt: &[u8],
    ) -> AccountId {
        let address = evm_contract_address(deploying_address, code_hash, input_data, salt);
        <Runtime as pallet_contracts_evm::Config>::AddressMapping::get_account_id(&address)
    }
}

#[derive_impl(pallet_contracts::config_preludes::TestDefaultConfig)]
impl pallet_contracts::Config for Runtime {
    type AddressGenerator = EvmAddressGenerator;
    type CallStack = [pallet_contracts::Frame<Self>; 5];
    type DepositPerByte = DepositPerByte;
    type DepositPerItem = DepositPerItem;
    type Currency = Balances;
    type Schedule = ContractSchedule;
    type Time = Timestamp;
}

parameter_types! {
  pub const ChainId: u64 = 596u64;
}

// TODO integrity check with tokenDecimals?
// Ideally this becomes a pallet::constant of pallet_balances pallet so that we don't need to hard
// code it here.
pub const NATIVE_DECIMALS: u32 = 15;

impl pallet_contracts_evm::Config for Runtime {
    type AddressMapping = EVMAddressMapping<Self>;
    type WeightInfo = pallet_contracts_evm::weights::SubstrateWeight<Self>;
    type EvmDecimalsConverter =
        pallet_contracts_evm::Converter<{ 10u64.pow(EVM_DECIMALS - NATIVE_DECIMALS) }, Balance>;
    type ChainId = ChainId;
}

/// An implementation of the [`Convert`] trait, used to extract an [`TransactionUnsigned`] Ethereum
/// transaction and a source [`H160`] address from an extrinsic.
/// This is used to check that an UncheckedExtrinsic that carry an Ethereum signature is valid.
#[derive(CloneNoBound, PartialEqNoBound, EqNoBound, DebugNoBound)]
pub struct ConvertEthTx<T: WeightToFee>(core::marker::PhantomData<T>);

/// Custom [`InvalidTransaction`] error code for fee mismatch.
pub const FEE_MISMATCH: u8 = 1;

impl<T: WeightToFee<Balance: Into<u128>>> ConvertEthTx<T> {
    /// Ensure that the fees defined in the transaction (`gas_price` * `gas_limit`) cover the fees
    /// calculated by the runtime, using the provided weight and storage deposit limit.
    fn validate_fee(
        eth_gas_price: u64,
        eth_gas_limit: u128,
        weight: Weight,
        storage_deposit_limit: Balance,
    ) -> Result<(), InvalidTransaction> {
        let eth_fee = eth_gas_limit * (eth_gas_price as u128);
        let actual_fee =
            T::weight_to_fee(&weight).into() + Into::<u128>::into(storage_deposit_limit);

        if eth_fee < actual_fee {
            log::trace!("Eth Fee do not match calculated fee: eth_fee: {eth_fee:?}, actual_fee: {actual_fee:?}");
            return Err(InvalidTransaction::Custom(FEE_MISMATCH));
        }
        Ok(())
    }

    /// Validate the [`SignedExtra`] and return the nonce.
    fn validate_extra(extra: SignedExtra) -> Result<U256, InvalidTransaction> {
        let (
            _check_non_zero,
            _check_spec_version,
            _check_tx_version,
            _check_genesis,
            mortality,
            check_nonce,
            _check_weight,
            _check_charge,
        ) = extra;

        // require immortal
        if mortality != frame_system::CheckEra::from(sp_runtime::generic::Era::Immortal) {
            return Err(InvalidTransaction::BadProof);
        }

        Ok(check_nonce.0.into())
    }
}
impl<T: WeightToFee<Balance: Into<u128>>>
    Convert<
        (RuntimeCall, SignedExtra),
        Result<(TransactionUnsigned, H160, SignedExtra), InvalidTransaction>,
    > for ConvertEthTx<T>
{
    fn convert(
        (call, extra): (RuntimeCall, SignedExtra),
    ) -> Result<(TransactionUnsigned, H160, SignedExtra), InvalidTransaction> {
        match call {
            RuntimeCall::ContractsEvm(pallet_contracts_evm::Call::eth_instantiate {
                source,
                code,
                data,
                salt,
                eth_value,
                eth_gas_price,
                eth_gas_limit,
                gas_limit,
                storage_deposit_limit,
            }) => {
                let source = source.ok_or(InvalidTransaction::BadProof)?;

                Self::validate_fee(
                    eth_gas_price,
                    eth_gas_limit,
                    gas_limit,
                    storage_deposit_limit.map(|x| x.into()).unwrap_or_default(),
                )?;

                let nonce = Self::validate_extra(extra.clone())?;
                let chain_id = <Runtime as pallet_contracts_evm::Config>::ChainId::get().into();
                let tx = TransactionLegacyUnsigned::from_instantiate(
                    CallInput { code, data, salt },
                    eth_value.into(),
                    eth_gas_price.into(),
                    eth_gas_limit.into(),
                    nonce,
                    chain_id,
                );
                Ok((tx.into(), source, extra))
            },
            RuntimeCall::ContractsEvm(pallet_contracts_evm::Call::eth_call {
                source,
                to,
                data: input,
                eth_value,
                eth_gas_price,
                eth_gas_limit,
                gas_limit,
                storage_deposit_limit,
            }) => {
                let source = source.ok_or(InvalidTransaction::BadProof)?;

                Self::validate_fee(
                    eth_gas_price,
                    eth_gas_limit,
                    gas_limit,
                    storage_deposit_limit.map(|x| x.into()).unwrap_or_default(),
                )?;
                let nonce = Self::validate_extra(extra.clone())?;
                let chain_id = <Runtime as pallet_contracts_evm::Config>::ChainId::get().into();
                let tx = TransactionLegacyUnsigned::from_call(
                    to,
                    input,
                    eth_value.into(),
                    eth_gas_price.into(),
                    eth_gas_limit.into(),
                    nonce,
                    chain_id,
                );

                Ok((tx.into(), source, extra))
            },
            _ => Err(InvalidTransaction::Call),
        }
    }
}

type WeightToFeeOf<T> = <T as pallet_transaction_payment::Config>::WeightToFee;
type Header = generic::Header<BlockNumber, BlakeTwo256>;
type Block = generic::Block<
    Header,
    UncheckedExtrinsic<RuntimeCall, SignedExtra, ConvertEthTx<WeightToFeeOf<Runtime>>>,
>;

type RuntimeExecutive =
    Executive<Runtime, Block, frame_system::ChainContext<Runtime>, Runtime, AllPalletsWithSystem>;

type EventRecord = frame_system::EventRecord<
    <Runtime as frame_system::Config>::RuntimeEvent,
    <Runtime as frame_system::Config>::Hash,
>;

use pallet_contracts_evm::AddressMapping;
impl_runtime_apis! {
  impl pallet_contracts_evm::ContractsEvmApi<Block, AccountId, interface::Balance> for Runtime {

    fn account_id(address: &H160) -> AccountId {
      <Runtime as pallet_contracts_evm::Config>::AddressMapping::get_account_id(address)
    }

    fn gas_estimate(transaction: GenericTransaction) -> Result<DryRunInfo<Balance>, DispatchError> {
      ContractsEvm::gas_estimate(transaction)
    }
  }

  impl pallet_contracts::ContractsApi<Block, AccountId, interface::Balance, BlockNumber, interface::Hash, EventRecord> for Runtime
  {
    fn call(
      origin: AccountId,
      dest: AccountId,
      value: interface::Balance,
      gas_limit: Option<Weight>,
      storage_deposit_limit: Option<interface::Balance>,
      input_data: Vec<u8>,
    ) -> pallet_contracts::ContractExecResult<interface::Balance, EventRecord> {
      let blockweights: BlockWeights = <Runtime as frame_system::Config>::BlockWeights::get();
      let gas_limit = gas_limit.unwrap_or(blockweights.max_block);
      Contracts::bare_call(
        origin,
        dest,
        value,
        gas_limit,
        storage_deposit_limit,
        input_data,
        pallet_contracts::DebugInfo::UnsafeDebug,
        pallet_contracts::CollectEvents::UnsafeCollect,
        pallet_contracts::Determinism::Enforced,
      )
    }

    fn instantiate(
      origin: AccountId,
      value: interface::Balance,
      gas_limit: Option<Weight>,
      storage_deposit_limit: Option<interface::Balance>,
      code: pallet_contracts::Code<interface::Hash>,
      data: Vec<u8>,
      salt: Vec<u8>,
    ) -> pallet_contracts::ContractInstantiateResult<AccountId, interface::Balance, EventRecord>
    {
      log::info!("dry-run instantiate");
      let blockweights: BlockWeights = <Runtime as frame_system::Config>::BlockWeights::get();
      let gas_limit = gas_limit.unwrap_or(blockweights.max_block);
      Contracts::bare_instantiate(
        origin,
        value,
        gas_limit,
        storage_deposit_limit,
        code,
        data,
        salt,
        pallet_contracts::DebugInfo::UnsafeDebug,
        pallet_contracts::CollectEvents::UnsafeCollect,
      )
    }

    fn upload_code(
      origin: AccountId,
      code: Vec<u8>,
      storage_deposit_limit: Option<interface::Balance>,
      determinism: pallet_contracts::Determinism,
    ) -> pallet_contracts::CodeUploadResult<interface::Hash, interface::Balance>
    {
      Contracts::bare_upload_code(
        origin,
        code,
        storage_deposit_limit,
        determinism,
      )
    }

    fn get_storage(
      address: AccountId,
      key: Vec<u8>,
    ) -> pallet_contracts::GetStorageResult {
      Contracts::get_storage(
        address,
        key
      )
    }
  }

  impl apis::Core<Block> for Runtime {
    fn version() -> RuntimeVersion {
      VERSION
    }

    fn execute_block(block: Block) {
      RuntimeExecutive::execute_block(block)
    }

    fn initialize_block(header: &Header) -> ExtrinsicInclusionMode {
      RuntimeExecutive::initialize_block(header)
    }
  }
  impl apis::Metadata<Block> for Runtime {
    fn metadata() -> OpaqueMetadata {
      OpaqueMetadata::new(Runtime::metadata().into())
    }

    fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
      Runtime::metadata_at_version(version)
    }

    fn metadata_versions() -> Vec<u32> {
      Runtime::metadata_versions()
    }
  }

  impl apis::BlockBuilder<Block> for Runtime {
    fn apply_extrinsic(extrinsic: ExtrinsicFor<Runtime>) -> ApplyExtrinsicResult {
      RuntimeExecutive::apply_extrinsic(extrinsic)
    }

    fn finalize_block() -> HeaderFor<Runtime> {
      RuntimeExecutive::finalize_block()
    }

    fn inherent_extrinsics(data: InherentData) -> Vec<ExtrinsicFor<Runtime>> {
      data.create_extrinsics()
    }

    fn check_inherents(
      block: Block,
      data: InherentData,
    ) -> CheckInherentsResult {
      data.check_extrinsics(&block)
    }
  }

  impl apis::TaggedTransactionQueue<Block> for Runtime {
    fn validate_transaction(
      source: TransactionSource,
      tx: ExtrinsicFor<Runtime>,
      block_hash: <Runtime as frame_system::Config>::Hash,
    ) -> TransactionValidity {
      RuntimeExecutive::validate_transaction(source, tx, block_hash)
    }
  }

  impl apis::OffchainWorkerApi<Block> for Runtime {
    fn offchain_worker(header: &HeaderFor<Runtime>) {
      RuntimeExecutive::offchain_worker(header)
    }
  }

  impl apis::SessionKeys<Block> for Runtime {
    fn generate_session_keys(_seed: Option<Vec<u8>>) -> Vec<u8> {
      Default::default()
    }

    fn decode_session_keys(
      _encoded: Vec<u8>,
    ) -> Option<Vec<(Vec<u8>, apis::KeyTypeId)>> {
      Default::default()
    }
  }

  impl apis::AccountNonceApi<Block, interface::AccountId, interface::Nonce> for Runtime {
    fn account_nonce(account: interface::AccountId) -> interface::Nonce {
      System::account_nonce(account)
    }
  }

  impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
    Block,
    interface::Balance,
  > for Runtime {
    fn query_info(uxt: ExtrinsicFor<Runtime>, len: u32) -> RuntimeDispatchInfo<interface::Balance> {
      TransactionPayment::query_info(uxt, len)
    }
    fn query_fee_details(uxt: ExtrinsicFor<Runtime>, len: u32) -> FeeDetails<interface::Balance> {
      TransactionPayment::query_fee_details(uxt, len)
    }
    fn query_weight_to_fee(weight: Weight) -> interface::Balance {
      TransactionPayment::weight_to_fee(weight)
    }
    fn query_length_to_fee(length: u32) -> interface::Balance {
      TransactionPayment::length_to_fee(length)
    }
  }

  impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
    fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
      build_state::<RuntimeGenesisConfig>(config)
    }

    fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
      get_preset::<RuntimeGenesisConfig>(id, |_| None)
    }

    fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
      vec![]
    }
  }
}

/// Some re-exports that the node side code needs to know. Some are useful in this context as well.
pub mod interface {
    use super::Runtime;
    use frame::deps::frame_system;

    pub type Block = super::Block;
    pub use frame::runtime::types_common::OpaqueBlock;
    pub type AccountId = <Runtime as frame_system::Config>::AccountId;
    pub type Nonce = <Runtime as frame_system::Config>::Nonce;
    pub type Hash = <Runtime as frame_system::Config>::Hash;
    pub type Balance = <Runtime as pallet_balances::Config>::Balance;
    pub type MinimumBalance = <Runtime as pallet_balances::Config>::ExistentialDeposit;
}
