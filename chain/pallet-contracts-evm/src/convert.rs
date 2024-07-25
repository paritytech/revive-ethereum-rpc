//! EVM decimals converter trait.
use core::marker::PhantomData;

use core::fmt::Debug;
use frame::{arithmetic::CheckedRem, deps::sp_runtime};
use sp_runtime::traits::{CheckedDiv, Saturating, Zero};

/// EVM decimals converter trait.
/// Convert decimal from native token to EVM(18).
pub trait EvmDecimalsConverter<
    Balance: Zero + Saturating + CheckedDiv + CheckedRem + PartialEq + Copy,
>
{
    /// Convert decimal from native token to EVM(18).
    fn convert_decimals_to_evm(native_balance: Balance) -> Balance;
    /// Convert decimal from EVM(18) to native.
    fn convert_decimals_from_evm(evm_balance: Balance) -> Option<Balance>;
}

/// EVM uses 18 decimals.
pub const EVM_DECIMALS: u32 = 18;

/// An [`EvmDecimalsConverter`] implementation
pub struct Converter<const DECIMALS_VALUE: u64, Balance>(PhantomData<Balance>);

impl<const DECIMALS_VALUE: u64, Balance> EvmDecimalsConverter<Balance>
    for Converter<DECIMALS_VALUE, Balance>
where
    Balance: Zero + Saturating + CheckedDiv + CheckedRem + PartialEq + Copy + From<u64> + Debug,
{
    fn convert_decimals_to_evm(b: Balance) -> Balance {
        if b.is_zero() {
            return b;
        }
        b.saturating_mul(DECIMALS_VALUE.into())
    }

    /// Convert decimal from EVM(18) to native
    fn convert_decimals_from_evm(b: Balance) -> Option<Balance> {
        if b.is_zero() {
            return Some(b);
        }
        if !b
            .checked_rem(&Into::<Balance>::into(DECIMALS_VALUE))?
            .is_zero()
        {
            log::error!("Failed to convert EVM balance: {b:?}, ratio: {DECIMALS_VALUE:?}");
            return None;
        }

        b.checked_div(&Into::<Balance>::into(DECIMALS_VALUE))
    }
}

#[test]
fn convert_works() {
    const NATIVE_DECIMALS: u32 = 15;
    type C = Converter<{ 10u64.pow(EVM_DECIMALS - NATIVE_DECIMALS) }, u128>;
    let native_value = 1_000;

    let evm_value = C::convert_decimals_to_evm(native_value);
    assert_eq!(evm_value, 1_000_000);
    assert_eq!(
        native_value,
        C::convert_decimals_from_evm(evm_value).unwrap()
    );
    assert!(C::convert_decimals_from_evm(evm_value + 1).is_none());

    assert_eq!(C::convert_decimals_to_evm(0), 0);
    assert_eq!(C::convert_decimals_from_evm(0).unwrap(), 0);
}
