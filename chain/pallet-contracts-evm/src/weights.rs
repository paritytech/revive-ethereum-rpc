use core::marker::PhantomData;

use frame::{deps::frame_system, prelude::Weight};
pub trait WeightInfo {
    fn eth_transfer() -> Weight {
        Weight::from_parts(280_000_000, 4_000)
    }
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {}
