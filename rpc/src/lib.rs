//! The [`EthRpcServer`] RPC server implementation
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

use adapters::CallInput;
use client::{ClientError, GAS_PRICE};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::{ErrorCode, ErrorObjectOwned},
};
use parity_scale_codec::Decode;
use primitives::MultiSignature;
use subxt::{config::DefaultExtrinsicParamsBuilder, tx::Signer, utils::MultiAddress};
use subxt_client::{runtime_types::sp_weights::weight_v2::Weight, SrcChainConfig};
use thiserror::Error;
pub mod client;
pub mod example;

mod rpc_methods;
pub use rpc_methods::*;

pub mod subxt_client;
mod tests;

use eth_rpc_api::*;

/// Additional RPC methods, exposed on the RPC server on top of all the eth_xxx methods.
#[rpc(server, client)]
pub trait MiscRpc {
    /// Returns the health status of the server.
    #[method(name = "healthcheck")]
    async fn healthcheck(&self) -> RpcResult<()>;
}

/// An EVM RPC server implementation.
pub struct EthRpcServerImpl {
    client: client::Client,
}

impl EthRpcServerImpl {
    /// Creates a new [`EthRpcServerImpl`].
    pub fn new(client: client::Client) -> Self {
        Self { client }
    }
}

/// The error type for the EVM RPC server.
#[derive(Error, Debug)]
pub enum EthRpcError {
    /// A [`ClientError`] wrapper error.
    #[error("Client error: {0}")]
    ClientError(#[from] ClientError),
    /// A [`rlp::DecoderError`] wrapper error.
    #[error("Decoding error: {0}")]
    RlpError(#[from] rlp::DecoderError),
    /// A Decimals conversion error.
    #[error("Conversion error")]
    ConversionError,
    /// An invalid signature error.
    #[error("Invalid signature")]
    InvalidSignature,
    /// The account was not found at the given address
    #[error("Account not found for address {0:?}")]
    AccountNotFound(H160),
}

impl From<EthRpcError> for ErrorObjectOwned {
    fn from(value: EthRpcError) -> Self {
        let code = match value {
            EthRpcError::ClientError(_) => ErrorCode::InternalError,
            _ => ErrorCode::InvalidRequest,
        };
        Self::owned::<String>(code.code(), value.to_string(), None)
    }
}

#[async_trait]
impl EthRpcServer for EthRpcServerImpl {
    async fn net_version(&self) -> RpcResult<String> {
        Ok(self.client.chain_id().to_string())
    }

    async fn block_number(&self) -> RpcResult<U256> {
        let number = self.client.block_number().await?;
        log::debug!("block_number: {number:?}");
        Ok(number.into())
    }

    async fn get_transaction_receipt(
        &self,
        transaction_hash: H256,
    ) -> RpcResult<Option<ReceiptInfo>> {
        let receipt = self.client.receipt(&transaction_hash).await;
        log::debug!("receipt: {receipt:#?}");
        Ok(receipt)
    }

    async fn estimate_gas(
        &self,
        transaction: GenericTransaction,
        _block: Option<BlockNumberOrTag>,
    ) -> RpcResult<U256> {
        let result = self
            .client
            .gas_estimate(&transaction, BlockTag::Latest.into())
            .await?;
        log::debug!("estimate_gas: for {transaction:#?} result = {result:?}");
        Ok(result)
    }

    async fn send_raw_transaction(&self, transaction: Bytes) -> RpcResult<H256> {
        let tx = rlp::decode::<TransactionLegacySigned>(&transaction.0).map_err(|err| {
            log::debug!("Failed to decode transaction: {err:?}");
            EthRpcError::from(err)
        })?;

        log::debug!("Decoded tx: {tx:?}");
        let eth_addr = tx.recover_eth_address().map_err(|err| {
            log::debug!("Failed to recover eth address: {err:?}");
            EthRpcError::InvalidSignature
        })?;

        let signature = MultiSignature::Ethereum(tx.raw_signature().map_err(|err| {
            log::debug!("Failed to extract signature: {err:?}");
            EthRpcError::InvalidSignature
        })?);

        let TransactionLegacyUnsigned {
            to,
            input,
            value,
            nonce,
            gas_price: eth_gas_price,
            gas: eth_gas_limit,
            ..
        } = tx.transaction_legacy_unsigned;

        let limits = self
            .client
            .dry_run(
                &GenericTransaction {
                    from: Some(eth_addr),
                    input: Some(input.clone()),
                    nonce: Some(nonce),
                    to,
                    value: Some(value),
                    ..Default::default()
                },
                BlockTag::Latest.into(),
            )
            .await?;

        let gas_limit = Weight {
            ref_time: limits.gas_limit.ref_time(),
            proof_size: limits.gas_limit.proof_size(),
        };

        let storage_deposit_limit = limits.storage_deposit_limit.map(|limit| limit.into());
        let account_id = self.client.account_id(&eth_addr);
        let params = DefaultExtrinsicParamsBuilder::default().build();
        let signer = EthSigner {
            account_id,
            signature,
        };

        let hash = if let Some(to) = to {
            let call = subxt_client::tx().contracts_evm().eth_call(
                Some(eth_addr),
                to,
                input.0,
                value.try_into().map_err(|_| {
                    log::debug!("Failed to convert call value: {value} to u64.");
                    EthRpcError::ConversionError
                })?,
                eth_gas_price.try_into().map_err(|_| {
                    log::debug!("Failed to convert call gas_price: {eth_gas_price} to u64.");
                    EthRpcError::ConversionError
                })?,
                eth_gas_limit.try_into().map_err(|_| {
                    log::debug!("Failed to convert call gas_limit: {eth_gas_limit} to u128.");
                    EthRpcError::ConversionError
                })?,
                gas_limit,
                storage_deposit_limit,
            );

            self.client
                .tx()
                .sign_and_submit(&call, &signer, params)
                .await
                .map_err(|err| EthRpcError::ClientError(err.into()))?
        } else {
            let CallInput { code, data, salt } =
                CallInput::decode(&mut &input.0[..]).map_err(|err| {
                    log::debug!("Failed to decode input: {err:?}");
                    ClientError::from(err)
                })?;

            let call = subxt_client::tx().contracts_evm().eth_instantiate(
                Some(eth_addr),
                code,
                data,
                salt,
                value.try_into().map_err(|_| {
                    log::debug!("Failed to convert instantiate value: {value} to Balance.");
                    EthRpcError::ConversionError
                })?,
                eth_gas_price.try_into().map_err(|_| {
                    log::debug!(
                        "Failed to convert instantiate gas_price {eth_gas_price} to Balance."
                    );
                    EthRpcError::ConversionError
                })?,
                eth_gas_limit.try_into().map_err(|_| {
                    log::debug!("Failed to convert instantiate gas_limit {eth_gas_limit} to u128.");
                    EthRpcError::ConversionError
                })?,
                gas_limit,
                storage_deposit_limit,
            );

            self.client
                .tx()
                .sign_and_submit(&call, &signer, params)
                .await
                .map_err(|err| {
                    log::debug!("Failed to submit instantiate call: {err:?}");
                    EthRpcError::ClientError(err.into())
                })?
        };

        log::debug!("send_raw_transaction succeed with hash: {hash}");
        Ok(hash)
    }

    async fn get_block_by_hash(
        &self,
        block_hash: H256,
        _hydrated_transactions: bool,
    ) -> RpcResult<Option<Block>> {
        let Some(block) = self.client.block_by_hash(&block_hash).await? else {
            return Ok(None);
        };
        let block = self.client.evm_block(block).await?;
        Ok(Some(block))
    }

    async fn get_balance(
        &self,
        address: Address,
        block: BlockNumberOrTagOrHash,
    ) -> RpcResult<U256> {
        let balance = self.client.balance(address, &block).await?;
        log::debug!("balance({address}): {balance:?}");
        Ok(balance)
    }

    async fn chain_id(&self) -> RpcResult<U256> {
        Ok(self.client.chain_id().into())
    }

    async fn gas_price(&self) -> RpcResult<U256> {
        Ok(U256::from(GAS_PRICE))
    }

    async fn get_code(&self, address: Address, block: BlockNumberOrTagOrHash) -> RpcResult<Bytes> {
        let code = self.client.get_contract_code(&address, block).await?;
        Ok(code.into())
    }

    async fn accounts(&self) -> RpcResult<Vec<Address>> {
        Ok(vec![])
    }

    async fn call(
        &self,
        transaction: GenericTransaction,
        block: Option<BlockNumberOrTagOrHash>,
    ) -> RpcResult<Bytes> {
        let info = self
            .client
            .dry_run(
                &transaction,
                block.unwrap_or_else(|| BlockTag::Latest.into()),
            )
            .await?;
        Ok(info.return_data.into())
    }

    async fn get_block_by_number(
        &self,
        block: BlockNumberOrTag,
        _hydrated_transactions: bool,
    ) -> RpcResult<Option<Block>> {
        let Some(block) = self.client.block_by_number_or_tag(&block).await? else {
            return Ok(None);
        };
        let block = self.client.evm_block(block).await?;
        Ok(Some(block))
    }

    async fn get_block_transaction_count_by_hash(
        &self,
        block_hash: Option<H256>,
    ) -> RpcResult<Option<U256>> {
        let block_hash = if let Some(block_hash) = block_hash {
            block_hash
        } else {
            self.client
                .latest_block()
                .await
                .ok_or(ClientError::BlockNotFound)?
                .hash()
        };
        Ok(self
            .client
            .receipts_count_per_block(&block_hash)
            .await
            .map(U256::from))
    }

    async fn get_block_transaction_count_by_number(
        &self,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<Option<U256>> {
        let Some(block) = self
            .get_block_by_number(block.unwrap_or_else(|| BlockTag::Latest.into()), false)
            .await?
        else {
            return Ok(None);
        };

        Ok(self
            .client
            .receipts_count_per_block(&block.hash)
            .await
            .map(U256::from))
    }

    async fn get_storage_at(
        &self,
        address: Address,
        storage_slot: U256,
        block: BlockNumberOrTagOrHash,
    ) -> RpcResult<Bytes> {
        let bytes = self
            .client
            .get_contract_storage(address, storage_slot, block)
            .await?;
        Ok(bytes.into())
    }

    async fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: H256,
        transaction_index: U256,
    ) -> RpcResult<Option<TransactionInfo>> {
        let receipt = self
            .client
            .receipt_by_hash_and_index(&block_hash, &transaction_index)
            .await;
        Ok(receipt.map(Into::into))
    }

    async fn get_transaction_by_block_number_and_index(
        &self,
        block: BlockNumberOrTag,
        transaction_index: U256,
    ) -> RpcResult<Option<TransactionInfo>> {
        let Some(block) = self.client.block_by_number_or_tag(&block).await? else {
            return Ok(None);
        };
        self.get_transaction_by_block_hash_and_index(block.hash(), transaction_index)
            .await
    }

    async fn get_transaction_by_hash(
        &self,
        transaction_hash: H256,
    ) -> RpcResult<Option<TransactionInfo>> {
        let receipt = self.client.receipt(&transaction_hash).await;
        Ok(receipt.map(Into::into))
    }

    async fn get_transaction_count(
        &self,
        address: Address,
        block: BlockNumberOrTagOrHash,
    ) -> RpcResult<U256> {
        let nonce = self.client.nonce(address, block).await?;
        Ok(nonce.into())
    }
}

/// A [`MiscRpcServer`] RPC server implementation.
pub struct MiscRpcServerImpl;

#[async_trait]
impl MiscRpcServer for MiscRpcServerImpl {
    async fn healthcheck(&self) -> RpcResult<()> {
        Ok(())
    }
}

/// A custom  subxt [`Signer`] that simply provide the existing Ethereum signature.
/// Once <https://github.com/paritytech/subxt/pull/1658> is merged we can just use
/// a [`subxt::tx::PartialExtrinsic`] and call `sign_with_address_and_signature` on it.
struct EthSigner {
    account_id: <SrcChainConfig as subxt::Config>::AccountId,
    signature: <SrcChainConfig as subxt::Config>::Signature,
}

impl Signer<SrcChainConfig> for EthSigner {
    fn account_id(&self) -> <SrcChainConfig as subxt::Config>::AccountId {
        self.account_id.clone()
    }

    fn address(&self) -> <SrcChainConfig as subxt::Config>::Address {
        MultiAddress::Id(self.account_id.clone())
    }

    fn sign(&self, _signer_payload: &[u8]) -> <SrcChainConfig as subxt::Config>::Signature {
        self.signature.clone()
    }
}
