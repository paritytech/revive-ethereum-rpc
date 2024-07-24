//! Ethereum Typed Transaction tyes
use frame::deps::codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::Byte;

/// A macro to generate Transaction type identifiers
/// See https://ethereum.org/en/developers/docs/transactions/#typed-transaction-envelope
macro_rules! transaction_type {
    ($name:ident, $value:literal) => {
        #[doc = concat!("Transaction type identifier: ", $value)]
        #[derive(Clone, Default, Debug, Eq, PartialEq)]
        pub struct $name;

        impl $name {
            pub fn as_byte(&self) -> Byte {
                Byte::from($value)
            }
        }

        impl Encode for $name {
            fn using_encoded<R, F: FnOnce(&[u8]) -> R>(&self, f: F) -> R {
                f(&[$value])
            }
        }
        impl Decode for $name {
            fn decode<I: parity_scale_codec::Input>(
                input: &mut I,
            ) -> Result<Self, parity_scale_codec::Error> {
                if $value == input.read_byte()? {
                    Ok(Self {})
                } else {
                    Err(parity_scale_codec::Error::from(concat!("expected ", $value)))
                }
            }
        }

        impl TypeInfo for $name {
            type Identity = u8;
            fn type_info() -> scale_info::Type {
                <u8 as TypeInfo>::type_info()
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(concat!("0x", $value))
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let s: &str = Deserialize::deserialize(deserializer)?;
                if s == concat!("0x", $value) {
                    Ok($name {})
                } else {
                    Err(serde::de::Error::custom(concat!("expected ", $value)))
                }
            }
        }
    };
}

transaction_type!(Type0, 0);
transaction_type!(Type1, 1);
transaction_type!(Type2, 2);
