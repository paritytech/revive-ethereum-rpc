//! The generated subxt client.
use primitives::MultiSignature;
use subxt::config::{Config, PolkadotConfig, PolkadotExtrinsicParams};

#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
mod src_chain {}

pub enum SrcChainConfig {}
impl Config for SrcChainConfig {
    type Hash = <PolkadotConfig as Config>::Hash;
    type AccountId = <PolkadotConfig as Config>::AccountId;
    type Address = <PolkadotConfig as Config>::Address;
    type Signature = MultiSignature;
    type Hasher = <PolkadotConfig as Config>::Hasher;
    type Header = <PolkadotConfig as Config>::Header;
    type AssetId = <PolkadotConfig as Config>::AssetId;
    type ExtrinsicParams = PolkadotExtrinsicParams<Self>;
}

pub use src_chain::*;
