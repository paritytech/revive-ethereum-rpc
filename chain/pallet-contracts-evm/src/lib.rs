#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec};

use eth_rpc_api::{adapters::*, GenericTransaction};
use frame_system::RawOrigin;
use parity_scale_codec::{Codec, Decode, Encode, HasCompact};
use polkadot_sdk::pallet_contracts;
use polkadot_sdk::polkadot_sdk_frame::{
    deps::sp_runtime::MultiAddress,
    derive::DefaultNoBound,
    prelude::*,
    primitives::{H160, U256},
    runtime::apis,
    traits::{
        fungible::{Inspect, Mutate},
        tokens::Preservation,
    },
    traits::{LookupError, StaticLookup},
};

mod convert;
pub use convert::*;

use pallet_contracts::Code;
use primitives::{self, AccountIndex};

// Re-export all pallet parts, this is needed to properly import the pallet into the runtime.
pub use pallet::*;
pub mod weights;
use pallet_contracts::WeightInfo as PalletContractsWeightInfo;
use polkadot_sdk::polkadot_sdk_frame as frame;
use weights::WeightInfo;

type BalanceOf<T> = <<T as pallet_contracts::Config>::Currency as Inspect<
    <T as frame_system::Config>::AccountId,
>>::Balance;

/// A mapping between `AccountId` and `EvmAddress`.
pub trait AddressMapping<AccountId> {
    /// Returns the AccountId associated to the EVM address.
    fn get_account_id(evm: &H160) -> AccountId;
}

#[frame::pallet]
pub mod pallet {
    use pallet_contracts::{CollectEvents, DebugInfo, Determinism};
    use polkadot_sdk::frame_support::dispatch::PostDispatchInfo;

    use super::*;
    use crate::frame_system;

    #[pallet::error]
    pub enum Error<T> {
        /// Decimals conversion failed.
        DecimalsConversionFailed,
        /// Invalid EVM Transaction.
        InvalidTransaction,
    }

    #[pallet::config]
    #[pallet::disable_frame_system_supertrait_check]
    pub trait Config: frame_system::Config + pallet_contracts::Config {
        /// Chain ID of EVM.
        #[pallet::constant]
        type ChainId: Get<u64>;

        /// Converter for EVM decimals.
        type EvmDecimalsConverter: convert::EvmDecimalsConverter<BalanceOf<Self>>;

        /// Describes the weights of the dispatchables of this module
        type WeightInfo: WeightInfo;

        /// Mapping from address to account id.
        type AddressMapping: AddressMapping<Self::AccountId>;
    }

    #[pallet::genesis_config]
    #[derive(DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// List of accounts to be endowed some initial balance.
        pub endowed_accounts: Vec<(H160, BalanceOf<T>)>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            for (address, balance) in &self.endowed_accounts {
                let account_id = T::AddressMapping::get_account_id(address);
                log::trace!(target: "evm", "Endow eth_address: {address:?} account_id: {account_id:?} balance: {balance:?}");
                T::Currency::set_balance(&account_id, *balance);
            }
        }
    }

    /// When dispatching an extrinsic from an EVM account, the [`H160`] source address will be set. We need to ensure that it matches the derived [`H160`] address from the `origin`.
    /// When dispatching an extrinsic from an AccountId, the source should be `None` and will be derived from the `origin` by truncating the `account_id`.
    pub fn ensure_signed_source<
        T: Config,
        OuterOrigin: Into<Result<RawOrigin<T::AccountId>, OuterOrigin>>,
    >(
        origin: OuterOrigin,
        source: Option<H160>,
    ) -> Result<T::AccountId, frame::traits::BadOrigin> {
        let origin = ensure_signed(origin)?;
        if let Some(source) = source {
            let source_account_id = T::AddressMapping::get_account_id(&source);
            ensure!(origin == source_account_id, frame::traits::BadOrigin);
        }
        Ok(origin)
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T>
    where
        <BalanceOf<T> as HasCompact>::Type: Clone + Eq + PartialEq + Debug + TypeInfo + Encode,
        BalanceOf<T>: Into<u128> + TryFrom<U256>,
    {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as pallet_contracts::Config>::WeightInfo::instantiate_with_code(code.len() as u32, data.len() as u32, salt.len() as u32).saturating_add(*gas_limit))]
        pub fn eth_instantiate(
            origin: OriginFor<T>,
            source: Option<H160>,
            code: Vec<u8>,
            data: Vec<u8>,
            salt: Vec<u8>,
            #[pallet::compact] eth_value: BalanceOf<T>,
            #[pallet::compact] _eth_gas_price: u64, // checked by tx validation logic
            #[pallet::compact] _eth_gas_limit: u128, // checked by tx validation logic
            gas_limit: Weight,
            storage_deposit_limit: Option<<BalanceOf<T> as HasCompact>::Type>,
        ) -> DispatchResultWithPostInfo {
            let origin = ensure_signed_source::<T, _>(origin, source)?;
            let storage_deposit_limit = storage_deposit_limit.map(|v| v.into());
            log::trace!(target: "evm", "instantiate contract from {origin:?} source: {source:?}");

            let code_len = code.len() as u32;
            let data_len = data.len() as u32;
            let salt_len = salt.len() as u32;
            let value = T::EvmDecimalsConverter::convert_decimals_from_evm(eth_value)
                .ok_or(Error::<T>::DecimalsConversionFailed)
                .inspect_err(|err| {
                    log::trace!(target: "evm", "Invalid request, failed to convert value from EVM: eth_value: {eth_value:?}: {err:?}")
                })?;

            let exec = pallet_contracts::Pallet::<T>::bare_instantiate(
                origin,
                value,
                gas_limit,
                storage_deposit_limit,
                pallet_contracts::Code::Upload(code),
                data,
                salt,
                DebugInfo::Skip,
                CollectEvents::Skip,
            );
            log::trace!(target: "evm", "instantiate contract result: {:?}", exec.result);
            exec.result?;

            let post_info = PostDispatchInfo {
                actual_weight: Some(exec.gas_consumed.saturating_add(
                    <T as pallet_contracts::Config>::WeightInfo::instantiate_with_code(
                        code_len, data_len, salt_len,
                    ),
                )),
                pays_fee: Default::default(),
            };

            Ok(post_info)
        }
        #[pallet::call_index(1)]
        #[pallet::weight(<T as pallet_contracts::Config>::WeightInfo::call().saturating_add(*gas_limit))]
        pub fn eth_call(
            origin: OriginFor<T>,
            source: Option<H160>,
            to: H160,
            data: Vec<u8>,
            #[pallet::compact] eth_value: BalanceOf<T>,
            #[pallet::compact] _eth_gas_price: u64, // checked by tx validation logic
            #[pallet::compact] _eth_gas_limit: u128, // checked by tx validation logic
            gas_limit: Weight,
            storage_deposit_limit: Option<<BalanceOf<T> as HasCompact>::Type>,
        ) -> DispatchResultWithPostInfo {
            let origin = ensure_signed_source::<T, _>(origin, source)?;
            let storage_deposit_limit = storage_deposit_limit.map(|v| v.into());
            log::trace!(target: "evm", "dispatched eth_call: origin: {origin:?} source: {source:?} to: {to:?} value: {eth_value:?}");
            let value = T::EvmDecimalsConverter::convert_decimals_from_evm(eth_value)
                .ok_or(Error::<T>::DecimalsConversionFailed)
                .inspect_err(|err| {
                    log::trace!(target: "evm", "Invalid request, failed to convert value from EVM: eth_value: {eth_value:?}: {err:?}")
                })?;

            let dest = T::AddressMapping::get_account_id(&to);

            let Some(code_hash) = pallet_contracts::Pallet::<T>::code_hash(&dest) else {
                log::trace!(target: "evm", "eth_call contract {dest:?} does not exist, transferring value: {value:?} to: {dest:?})");
                T::Currency::transfer(&origin, &dest, value, Preservation::Preserve).inspect_err(
                    |err| log::trace!(target: "evm", "eth_call transfer failed: {err:?}"),
                )?;

                return Ok(PostDispatchInfo {
                    actual_weight: Some(<T as Config>::WeightInfo::eth_transfer()),
                    pays_fee: Default::default(),
                });
            };

            log::trace!(target: "evm", "call contract: {dest:?} hash: {code_hash:?}");
            let exec = pallet_contracts::Pallet::<T>::bare_call(
                origin,
                dest,
                value,
                gas_limit,
                storage_deposit_limit,
                data,
                DebugInfo::Skip,
                CollectEvents::Skip,
                Determinism::Enforced,
            );

            log::trace!(target: "evm", "call contract result: {:?}", exec.result);
            exec.result?;

            let post_info = PostDispatchInfo {
                actual_weight: Some(
                    exec.gas_consumed
                        .saturating_add(<T as pallet_contracts::Config>::WeightInfo::call()),
                ),
                pays_fee: Default::default(),
            };

            Ok(post_info)
        }
    }
}

apis::decl_runtime_apis! {

  pub trait ContractsEvmApi<AccountId, Balance> where
    AccountId: Codec,
    Balance: Codec,
  {
    /// Get the account id for the given EVM address.
    fn account_id(address: &H160) -> AccountId;

    /// Dry run a transaction and return the gas used.
    fn gas_estimate(transaction: GenericTransaction) -> Result<DryRunInfo<Balance>, DispatchError>;
  }
}

impl<T: Config> Pallet<T>
where
    BalanceOf<T>: Into<u128> + TryFrom<U256>,
{
    /// Convert the raw input data from an EVM transaction into a `CallInput` struct.
    pub fn split_input_data(input: Vec<u8>) -> Result<CallInput, DispatchError> {
        let input = CallInput::decode(&mut &input[..]).map_err(|_| {
            log::trace!(target: "evm", "eth_call decoding code & data failed");
            pallet_contracts::Error::<T>::DecodingFailed
        })?;
        Ok(input)
    }

    /// Dry run a transaction and return the result.
    pub fn gas_estimate(tx: GenericTransaction) -> Result<DryRunInfo<BalanceOf<T>>, DispatchError> {
        log::trace!(target: "evm", "Get gas estimate for address: {:?}", tx.from);
        let origin = tx
      .from
      .map(|from| T::AddressMapping::get_account_id(&from))
      .ok_or(Error::<T>::InvalidTransaction)
      .inspect_err(|_| {
        log::trace!(target: "evm", "Invalid request, failed to get account_id from EVM address: {:?}", tx.from);
      })?;

        log::trace!(target: "evm", "Get gas estimate account_id: {origin:?}");

        // check if it is a simple balance transfer
        if let Some(to) = tx.to {
            let to = T::AddressMapping::get_account_id(&to);
            match pallet_contracts::Pallet::<T>::code_hash(&to) {
                None => {
                    log::trace!(target: "evm", "eth_call contract {to:?} does not exist, dry run transfer...");
                    let info = DryRunInfo::<BalanceOf<T>> {
                        gas_limit: <T as Config>::WeightInfo::eth_transfer(),
                        storage_deposit_limit: None,
                        return_data: vec![],
                    };

                    return Ok(info);
                }
                Some(code_hash) => {
                    log::trace!(target: "evm", "eth_call contract {to:?} code_hash: {code_hash:?}");
                }
            }
        }

        let input_data = tx.input.map(|bytes| bytes.0).unwrap_or_default();
        log::trace!(target: "evm", "Get gas estimate for input.len() = {}", input_data.len());

        let value = tx
            .value
            .map(|v| BalanceOf::<T>::try_from(v).map_err(|_| Error::<T>::InvalidTransaction))
            .transpose()?
            .unwrap_or_default();

        log::trace!(target: "evm", "Get gas estimate value = {value:?}");
        let gas_limit = T::BlockWeights::get()
            .get(DispatchClass::Normal)
            .max_total
            .unwrap_or_else(|| T::BlockWeights::get().max_block);

        let storage_deposit_limit = None;

        let dry_run_info = match tx.to {
            None => {
                let CallInput { code, data, salt } = Self::split_input_data(input_data)?;
                let result = pallet_contracts::Pallet::<T>::bare_instantiate(
                    origin,
                    value,
                    gas_limit,
                    storage_deposit_limit,
                    Code::Upload(code),
                    data,
                    salt,
                    pallet_contracts::DebugInfo::UnsafeDebug,
                    pallet_contracts::CollectEvents::UnsafeCollect,
                );

                let exec_result = result.result.inspect_err(|err| {
                    log::trace!(target: "evm", "Get gas estimate failed {err:?}");
                })?;

                if exec_result.result.did_revert() {
                    log::trace!(target: "evm", "Get gas estimate failed, contract reverted");
                    return Err(pallet_contracts::Error::<T>::ContractReverted.into());
                }

                let return_data = exec_result.result.data;

                DryRunInfo::<BalanceOf<T>> {
                    gas_limit: result.gas_required,
                    storage_deposit_limit: Some(result.storage_deposit.charge_or_zero()),
                    return_data,
                }
            }
            Some(address) => {
                let dest = T::AddressMapping::get_account_id(&address);

                let result = pallet_contracts::Pallet::<T>::bare_call(
                    origin,
                    dest,
                    value,
                    gas_limit,
                    storage_deposit_limit,
                    input_data,
                    pallet_contracts::DebugInfo::UnsafeDebug,
                    pallet_contracts::CollectEvents::UnsafeCollect,
                    pallet_contracts::Determinism::Enforced,
                );

                let exec_result = result.result.inspect_err(|err| {
                    log::trace!(target: "evm", "Get gas estimate failed {err:?}");
                })?;

                if exec_result.did_revert() {
                    log::trace!(target: "evm", "Get gas estimate failed, contract reverted");
                    return Err(pallet_contracts::Error::<T>::ContractReverted.into());
                }

                DryRunInfo::<BalanceOf<T>> {
                    gas_limit: result.gas_required,
                    storage_deposit_limit: Some(result.storage_deposit.charge_or_zero()),
                    return_data: exec_result.data,
                }
            }
        };

        log::trace!(target: "evm", "gas_estimate: {dry_run_info:?}");
        Ok(dry_run_info)
    }
}

/// An implementation of [`AddressMapping`] that maps [`H160`] addresses to Substrate `AccountId`s.
pub struct EVMAddressMapping<T: Config>(PhantomData<T>);
impl<T: Config> AddressMapping<T::AccountId> for EVMAddressMapping<T>
where
    T::AccountId: IsType<primitives::AccountId>,
{
    fn get_account_id(evm: &H160) -> T::AccountId {
        primitives::get_account_id(evm).into()
    }
}

impl<T: Config> StaticLookup for Pallet<T> {
    type Source = MultiAddress<T::AccountId, AccountIndex>;
    type Target = T::AccountId;

    fn lookup(a: Self::Source) -> Result<Self::Target, LookupError> {
        match a {
            MultiAddress::Id(id) => Ok(id),
            MultiAddress::Address20(i) => {
                Ok(T::AddressMapping::get_account_id(&H160::from_slice(&i)))
            }
            _ => Err(LookupError),
        }
    }

    fn unlookup(a: Self::Target) -> Self::Source {
        MultiAddress::Id(a)
    }
}
