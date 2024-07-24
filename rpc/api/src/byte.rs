//! Define Byte wrapper types for encoding and decoding hex strings
use core::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    str::FromStr,
};
use frame::derive::{Decode, Encode, TypeInfo};
use serde::{Deserialize, Serialize};

#[cfg(not(feature = "std"))]
use alloc::vec;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use hex_serde::HexCodec;

mod hex_serde {
    #[cfg(not(feature = "std"))]
    use alloc::{format, string::String, vec::Vec};
    use serde::{Deserialize, Deserializer, Serializer};

    pub trait HexCodec: Sized {
        type Error;
        fn to_hex(&self) -> String;
        fn from_hex(s: String) -> Result<Self, Self::Error>;
    }

    impl HexCodec for u8 {
        type Error = core::num::ParseIntError;
        fn to_hex(&self) -> String {
            format!("0x{:x}", self)
        }
        fn from_hex(s: String) -> Result<Self, Self::Error> {
            u8::from_str_radix(s.trim_start_matches("0x"), 16)
        }
    }

    impl<const T: usize> HexCodec for [u8; T] {
        type Error = hex::FromHexError;
        fn to_hex(&self) -> String {
            format!("0x{}", hex::encode(self))
        }
        fn from_hex(s: String) -> Result<Self, Self::Error> {
            let data = hex::decode(s.trim_start_matches("0x"))?;
            data.try_into().map_err(|_| hex::FromHexError::InvalidStringLength)
        }
    }

    impl HexCodec for Vec<u8> {
        type Error = hex::FromHexError;
        fn to_hex(&self) -> String {
            format!("0x{}", hex::encode(self))
        }
        fn from_hex(s: String) -> Result<Self, Self::Error> {
            hex::decode(s.trim_start_matches("0x"))
        }
    }

    pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: HexCodec,
    {
        let s = value.to_hex();
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: HexCodec,
        <T as HexCodec>::Error: core::fmt::Debug,
    {
        let s = String::deserialize(deserializer)?;
        let value = T::from_hex(s).map_err(|e| serde::de::Error::custom(format!("{:?}", e)))?;
        Ok(value)
    }
}

impl FromStr for Bytes {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let data = hex::decode(s.trim_start_matches("0x"))?;
        Ok(Bytes(data))
    }
}

macro_rules! impl_hex {
    ($type:ident, $inner:ty, $default:expr) => {
        #[derive(Encode, Decode, Eq, PartialEq, TypeInfo, Clone, Serialize, Deserialize)]
        #[doc = concat!("`", stringify!($inner), "`", " wrapper type for encoding and decoding hex strings")]
        pub struct $type(#[serde(with = "hex_serde")] pub $inner);

        impl Default for $type {
            fn default() -> Self {
                $type($default)
            }
        }

        impl From<$inner> for $type {
            fn from(inner: $inner) -> Self {
                $type(inner)
            }
        }

        impl Debug for $type {
            fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
                write!(f, concat!(stringify!($type), "({})"), self.0.to_hex())
            }
        }

        impl Display for $type {
            fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
                write!(f, "{}", self.0.to_hex())
            }
        }
    };
}

impl_hex!(Byte, u8, 0u8);
impl_hex!(Bytes, Vec<u8>, vec![]);
impl_hex!(Bytes256, [u8; 256], [0u8; 256]);

#[test]
fn serialize_byte() {
    let a = Byte(42);
    let s = serde_json::to_string(&a).unwrap();
    assert_eq!(s, "\"0x2a\"");
    let b = serde_json::from_str::<Byte>(&s).unwrap();
    assert_eq!(a, b);
}
