//! The client connects to the source substrate chain
//! and is used by the rpc server to query and send transactions to the substrate chain.

use eth_rpc_api::{
    adapters::DryRunInfo, BlockNumberOrTag, BlockNumberOrTagOrHash, Bytes256, GenericTransaction,
    ReceiptInfo, H160, H256, U256,
};
use frame::{prelude::Weight, traits::Hash};
use futures::{stream, StreamExt};
use jsonrpsee::types::{ErrorCode, ErrorObjectOwned};
use parity_scale_codec::{Decode, Encode};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Duration,
};
use subxt::{
    backend::{
        legacy::LegacyRpcMethods,
        rpc::{
            reconnecting_rpc_client::{Client as ReconnectingRpcClient, ExponentialBackoff},
            RpcClient,
        },
    },
    config::Header,
    error::RpcError,
    storage::Storage,
    tx::TxClient,
    utils::AccountId32,
    Config, OnlineClient,
};
use subxt_client::transaction_payment::events::TransactionFeePaid;
use thiserror::Error;
use tokio::{
    sync::{watch::Sender, RwLock},
    task::JoinSet,
};

use crate::subxt_client::{
    self,
    contracts_evm::calls::types::{EthCall, EthInstantiate},
    system::events::ExtrinsicSuccess,
    SrcChainConfig,
};

/// The substrate block type.
pub type SubstrateBlock = subxt::blocks::Block<SrcChainConfig, OnlineClient<SrcChainConfig>>;

/// The substrate block number type.
pub type SubstrateBlockNumber = <<SrcChainConfig as Config>::Header as Header>::Number;

/// The substrate block hash type.
pub type SubstrateBlockHash = <SrcChainConfig as Config>::Hash;

/// Type alias for shared data.
pub type Shared<T> = Arc<RwLock<T>>;

/// The runtime balance type.
pub type Balance = u128;

/// The EVM gas price.
/// We use a fixed value for the gas price.
/// This let us calculate the gas estimate for a transaction with the formula:
/// `gas_estimate = substrate_fee / gas_price`.
pub const GAS_PRICE: u128 = 1u128;

/// The cache maintains a buffer of the last N blocks,
#[derive(Default)]
struct BlockCache<const N: usize> {
    /// A double-ended queue of the last N blocks.
    /// The most recent block is at the back of the queue, and the oldest block is at the front.
    buffer: VecDeque<Arc<SubstrateBlock>>,

    /// A map of blocks by block number.
    blocks_by_number: HashMap<SubstrateBlockNumber, Arc<SubstrateBlock>>,

    /// A map of blocks by block hash.
    blocks_by_hash: HashMap<H256, Arc<SubstrateBlock>>,

    /// A map of receipts by hash.
    receipts_by_hash: HashMap<H256, ReceiptInfo>,

    /// A map of receipt hashes by block hash.
    receipt_hashes_by_block_and_index: HashMap<H256, HashMap<U256, H256>>,
}

/// The `pallet_contracts_evm` extrinsics.
#[derive(Debug)]
enum EvmExtrinsic {
    /// The `pallet_contracts_evm::call` extrinsic.
    Instantiate {
        payload: EthInstantiate,
        address_bytes: Vec<u8>,
    },

    /// The `pallet_contracts_evm::instantiate` extrinsic.
    Call(EthCall),
}

/// The receipt info extracted from an extrinsic.
struct ExtrinsicReceiptInfo {
    from: H160,
    to: Option<H160>,
    contract_address: Option<H160>,
    gas_price: u64,
}

impl EvmExtrinsic {
    /// Extract the receipt info from the extrinsic.
    fn into_receipt_info(self) -> Option<ExtrinsicReceiptInfo> {
        match self {
            EvmExtrinsic::Call(call) => Some(ExtrinsicReceiptInfo {
                from: call.source?,
                to: Some(call.to),
                contract_address: None,
                gas_price: call.eth_gas_price,
            }),
            EvmExtrinsic::Instantiate {
                payload,
                address_bytes,
            } => {
                // Calculate the contract address from the payload
                let code_hash = primitives::Hashing::hash(&payload.code);
                let address = primitives::MultiAddress::decode(&mut address_bytes.as_ref()).ok()?;
                let address: primitives::AccountId = match address {
                    primitives::MultiAddress::Id(id) => id,
                    _ => return None,
                };
                let contract_address = primitives::evm_contract_address(
                    &address,
                    &code_hash,
                    &payload.data,
                    &payload.salt,
                );

                log::debug!("Add Contract receipt at address: {contract_address:?}");

                Some(ExtrinsicReceiptInfo {
                    from: payload.source?,
                    to: None,
                    contract_address: Some(contract_address),
                    gas_price: payload.eth_gas_price,
                })
            }
        }
    }
}

/// The error type for the client.
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Subxt error: {0}")]
    SubxtError(#[from] subxt::Error),
    #[error("RPC error: {0}")]
    RpcError(#[from] RpcError),
    #[error("Codec error: {0}")]
    CodecError(#[from] parity_scale_codec::Error),
    #[error("Dispatch error")]
    DispatchError,
    #[error("Conversion failed")]
    ConversionFailed,
    #[error("Hash not found")]
    BlockNotFound,
    #[error("TransactionFeePaid event not found")]
    TxFeeNotFound,
    #[error("tokenDecimals not found in properties")]
    TokenDecimalsNotFound,
    #[error("Cache is empty")]
    CacheEmpty,
}

// Convert a `ClientError` to an RPC `ErrorObjectOwned`.
impl From<ClientError> for ErrorObjectOwned {
    fn from(_value: ClientError) -> Self {
        ErrorObjectOwned::owned::<()>(
            ErrorCode::InternalError.code(),
            ErrorCode::InternalError.message(),
            None,
        )
    }
}

/// The number of recent blocks maintained by the cache.
/// For each block in the cache, we also store the EVM transaction receipts.
pub const CACHE_SIZE: usize = 10;

impl<const N: usize> BlockCache<N> {
    fn latest_block(&self) -> Option<&Arc<SubstrateBlock>> {
        self.buffer.back()
    }

    /// Insert an entry into the cache, and prune the oldest entry if the cache is full.
    fn insert(&mut self, block: SubstrateBlock) {
        if self.buffer.len() >= N {
            if let Some(block) = self.buffer.pop_front() {
                log::trace!("Pruning block: {}", block.number());
                let hash = block.hash();
                self.blocks_by_hash.remove(&hash);
                self.blocks_by_number.remove(&block.number());
                if let Some(entries) = self.receipt_hashes_by_block_and_index.remove(&hash) {
                    for hash in entries.values() {
                        self.receipts_by_hash.remove(hash);
                    }
                }
            }
        }

        let block = Arc::new(block);
        self.buffer.push_back(block.clone());
        self.blocks_by_number.insert(block.number(), block.clone());
        self.blocks_by_hash.insert(block.hash(), block);
    }
}

/// A client connect to a node and maintains a cache of the last `CACHE_SIZE` blocks.
pub struct Client {
    inner: Arc<ClientInner>,
    join_set: JoinSet<Result<(), ClientError>>,
    pub updates: tokio::sync::watch::Receiver<()>,
}

/// The inner state of the client.
struct ClientInner {
    api: OnlineClient<SrcChainConfig>,
    rpc_client: ReconnectingRpcClient,
    rpc: LegacyRpcMethods<SrcChainConfig>,
    cache: Shared<BlockCache<CACHE_SIZE>>,
    chain_id: u64,
    max_block_weight: Weight,
    native_to_evm_ratio: U256,
}

impl ClientInner {
    /// Create a new client instance connecting to the substrate node at the given URL.
    async fn from_url(url: &str) -> Result<Self, ClientError> {
        let rpc_client = ReconnectingRpcClient::builder()
            .retry_policy(ExponentialBackoff::from_millis(100).max_delay(Duration::from_secs(10)))
            .build(url.to_string())
            .await?;

        let api = OnlineClient::<SrcChainConfig>::from_rpc_client(rpc_client.clone()).await?;
        let cache = Arc::new(RwLock::new(BlockCache::<CACHE_SIZE>::default()));

        let rpc = LegacyRpcMethods::<SrcChainConfig>::new(RpcClient::new(rpc_client.clone()));

        let (native_to_evm_ratio, chain_id, max_block_weight) = tokio::try_join!(
            native_to_evm_ratio(&rpc),
            chain_id(&api),
            max_block_weight(&api)
        )?;

        Ok(Self {
            api,
            rpc_client,
            rpc,
            cache,
            chain_id,
            max_block_weight,
            native_to_evm_ratio,
        })
    }

    /// Convert a native balance to an EVM balance.
    pub fn native_to_evm_decimals(&self, value: U256) -> U256 {
        value.saturating_mul(self.native_to_evm_ratio)
    }

    /// Get the receipt infos from the extrinsics in a block.
    async fn receipt_infos(
        &self,
        block: &SubstrateBlock,
    ) -> Result<HashMap<H256, ReceiptInfo>, ClientError> {
        // Get extrinsics from the block
        let extrinsics = block.extrinsics().await?;

        // Filter extrinsics that are pallet_contracts_evm
        let extrinsics = extrinsics.iter().flat_map(|ext| {
            let ext = ext.ok()?;

            let payload: EvmExtrinsic =
                if let Ok(Some(payload)) = ext.as_extrinsic::<EthInstantiate>() {
                    EvmExtrinsic::Instantiate {
                        payload,
                        address_bytes: ext.address_bytes()?.into(),
                    }
                } else if let Ok(Some(payload)) = ext.as_extrinsic::<EthCall>() {
                    EvmExtrinsic::Call(payload)
                } else {
                    return None;
                };
            Some((payload.into_receipt_info()?, ext))
        });

        // Map each extrinsic to a receipt
        stream::iter(extrinsics)
            .map(
                |(
                    ExtrinsicReceiptInfo {
                        from,
                        to,
                        contract_address,
                        gas_price,
                    },
                    ext,
                )| async move {
                    let events = ext.events().await?;
                    let tx_fees = events
                        .find_first::<TransactionFeePaid>()?
                        .ok_or(ClientError::TxFeeNotFound)?;

                    let gas_used = (tx_fees.tip.saturating_add(tx_fees.actual_fee))
                        .checked_div(gas_price as _)
                        .unwrap_or_default();

                    let success = events.find_first::<ExtrinsicSuccess>().is_ok();
                    let transaction_index = ext.index();
                    let transaction_hash =
                        primitives::Hashing::hash(&Vec::from(ext.bytes()).encode());
                    let block_hash = block.hash();
                    let block_number = block.number().into();

                    let receipt = ReceiptInfo {
                        block_hash,
                        block_number,
                        contract_address,
                        effective_gas_price: gas_price.into(),
                        from,
                        gas_used: gas_used.into(),
                        to,
                        status: Some(if success { U256::one() } else { U256::zero() }),
                        transaction_hash,
                        transaction_index: transaction_index.into(),
                        ..Default::default()
                    };

                    Ok::<_, ClientError>((receipt.transaction_hash, receipt))
                },
            )
            .buffer_unordered(10)
            .collect::<Vec<Result<_, _>>>()
            .await
            .into_iter()
            .collect::<Result<HashMap<_, _>, _>>()
    }
}

/// Drop all the tasks spawned by the client on drop.
impl Drop for Client {
    fn drop(&mut self) {
        self.join_set.abort_all()
    }
}

/// Fetch the chain ID from the substrate chain.
async fn chain_id(api: &OnlineClient<SrcChainConfig>) -> Result<u64, ClientError> {
    let query = subxt_client::constants().contracts_evm().chain_id();
    api.constants().at(&query).map_err(|err| err.into())
}

/// Fetch the max block weight from the substrate chain.
async fn max_block_weight(api: &OnlineClient<SrcChainConfig>) -> Result<Weight, ClientError> {
    let query = subxt_client::constants().system().block_weights();
    let weights = api.constants().at(&query)?;
    let max_block = weights
        .per_class
        .normal
        .max_extrinsic
        .unwrap_or(weights.max_block);
    Ok(Weight::from_parts(max_block.ref_time, max_block.proof_size))
}

/// Fetch the native to EVM ratio from the substrate chain.
async fn native_to_evm_ratio(rpc: &LegacyRpcMethods<SrcChainConfig>) -> Result<U256, ClientError> {
    let props = rpc.system_properties().await?;
    let eth_decimals = U256::from(18u32);
    let native_decimals: U256 = props
        .get("tokenDecimals")
        .and_then(|v| v.as_number()?.as_u64())
        .ok_or(ClientError::TokenDecimalsNotFound)?
        .into();

    Ok(U256::from(10u32).pow(eth_decimals - native_decimals))
}

/// Extract the block timestamp.
async fn extract_block_timestamp(block: &SubstrateBlock) -> Option<u64> {
    let extrinsics = block.extrinsics().await.ok()?;
    let ext = extrinsics
        .find_first::<crate::subxt_client::timestamp::calls::types::Set>()
        .ok()??;

    Some(ext.value.now / 1000)
}

impl Client {
    /// Create a new client instance.
    /// The client will subscribe to new blocks and maintain a cache of [`CACHE_SIZE`] blocks.
    pub async fn from_url(url: &str) -> Result<Self, ClientError> {
        log::info!("Connecting to node at: {url} ...");
        let inner: Arc<ClientInner> = Arc::new(ClientInner::from_url(url).await?);
        log::info!("Connected to node at: {url}");

        let (tx, mut updates) = tokio::sync::watch::channel(());
        let mut join_set = JoinSet::new();
        join_set.spawn(Self::subscribe_blocks(inner.clone(), tx));
        join_set.spawn(Self::subscribe_reconnect(inner.clone()));

        updates.changed().await.expect("tx is not dropped");
        Ok(Self {
            inner,
            join_set,
            updates,
        })
    }

    /// Subscribe and log reconnection events.
    async fn subscribe_reconnect(inner: Arc<ClientInner>) -> Result<(), ClientError> {
        let rpc = inner.as_ref().rpc_client.clone();
        loop {
            let reconnected = rpc.reconnect_initiated().await;
            log::info!("RPC client connection lost");
            let now = std::time::Instant::now();
            reconnected.await;
            log::info!(
                "RPC client reconnection took `{}s`",
                now.elapsed().as_secs()
            );
        }
    }

    /// Subscribe to new blocks and update the cache.
    async fn subscribe_blocks(inner: Arc<ClientInner>, tx: Sender<()>) -> Result<(), ClientError> {
        log::info!("Subscribing to new blocks");
        let mut block_stream = inner
            .as_ref()
            .api
            .blocks()
            .subscribe_finalized()
            .await
            .inspect_err(|err| {
                log::error!("Failed to subscribe to blocks: {err:?}");
            })?;

        while let Some(block) = block_stream.next().await {
            let block = match block {
                Ok(block) => block,
                Err(err) => {
                    if err.is_disconnected_will_reconnect() {
                        log::warn!(
                            "The RPC connection was lost and we may have missed a few blocks"
                        );
                        continue;
                    }

                    log::error!("Failed to fetch block: {err:?}");
                    return Err(err.into());
                }
            };

            log::debug!("Pushing block: {}", block.number());
            let mut cache = inner.cache.write().await;

            let receipts = inner.receipt_infos(&block).await.inspect_err(|err| {
                log::error!("Failed to get receipts: {err:?}");
            })?;

            if !receipts.is_empty() {
                log::debug!("Adding {} receipts", receipts.len());
                let values = receipts
                    .iter()
                    .map(|(hash, receipt)| (receipt.transaction_index, *hash))
                    .collect::<HashMap<_, _>>();
                cache.receipts_by_hash.extend(receipts);
                cache
                    .receipt_hashes_by_block_and_index
                    .insert(block.hash(), values);
            }

            cache.insert(block);
            tx.send_replace(());
        }

        log::info!("Block subscription ended");
        Ok(())
    }

    /// Get the most recent block stored in the cache.
    pub async fn latest_block(&self) -> Option<Arc<SubstrateBlock>> {
        let cache = self.inner.cache.read().await;
        let block = cache.latest_block()?;
        Some(block.clone())
    }

    /// Expose the transaction API.
    pub fn tx(&self) -> TxClient<SrcChainConfig, OnlineClient<SrcChainConfig>> {
        self.inner.api.tx()
    }

    /// Get an EVM transaction receipt by hash.
    pub async fn receipt(&self, tx_hash: &H256) -> Option<ReceiptInfo> {
        let cache = self.inner.cache.read().await;
        cache.receipts_by_hash.get(tx_hash).cloned()
    }

    /// Get an EVM transaction receipt by hash.
    pub async fn receipt_by_hash_and_index(
        &self,
        block_hash: &H256,
        transaction_index: &U256,
    ) -> Option<ReceiptInfo> {
        let cache = self.inner.cache.read().await;
        let receipt_hash = cache
            .receipt_hashes_by_block_and_index
            .get(block_hash)?
            .get(transaction_index)?;
        let receipt = cache.receipts_by_hash.get(receipt_hash)?;
        Some(receipt.clone())
    }

    /// Get receipts count per block.
    pub async fn receipts_count_per_block(&self, block_hash: &SubstrateBlockHash) -> Option<usize> {
        let cache = self.inner.cache.read().await;
        cache
            .receipt_hashes_by_block_and_index
            .get(block_hash)
            .map(|v| v.len())
    }

    /// Expose the storage API.
    pub async fn storage_api(
        &self,
        at: &BlockNumberOrTagOrHash,
    ) -> Result<Storage<SrcChainConfig, OnlineClient<SrcChainConfig>>, ClientError> {
        match at {
            BlockNumberOrTagOrHash::U256(block_number) => {
                let n: SubstrateBlockNumber = (*block_number)
                    .try_into()
                    .map_err(|_| ClientError::ConversionFailed)?;

                let hash = self
                    .get_block_hash(n)
                    .await?
                    .ok_or(ClientError::BlockNotFound)?;
                Ok(self.inner.api.storage().at(hash))
            }
            BlockNumberOrTagOrHash::H256(hash) => Ok(self.inner.api.storage().at(*hash)),
            BlockNumberOrTagOrHash::BlockTag(_) => {
                if let Some(block) = self.latest_block().await {
                    return Ok(self.inner.api.storage().at(block.hash()));
                }
                let storage = self.inner.api.storage().at_latest().await?;
                Ok(storage)
            }
        }
    }

    /// Expose the runtime API.
    pub async fn runtime_api(
        &self,
        at: &BlockNumberOrTagOrHash,
    ) -> Result<
        subxt::runtime_api::RuntimeApi<SrcChainConfig, OnlineClient<SrcChainConfig>>,
        ClientError,
    > {
        match at {
            BlockNumberOrTagOrHash::U256(block_number) => {
                let n: SubstrateBlockNumber = (*block_number)
                    .try_into()
                    .map_err(|_| ClientError::ConversionFailed)?;

                let hash = self
                    .get_block_hash(n)
                    .await?
                    .ok_or(ClientError::BlockNotFound)?;
                Ok(self.inner.api.runtime_api().at(hash))
            }
            BlockNumberOrTagOrHash::H256(hash) => Ok(self.inner.api.runtime_api().at(*hash)),
            BlockNumberOrTagOrHash::BlockTag(_) => {
                if let Some(block) = self.latest_block().await {
                    return Ok(self.inner.api.runtime_api().at(block.hash()));
                }

                let api = self.inner.api.runtime_api().at_latest().await?;
                Ok(api)
            }
        }
    }

    /// Get the balance of the given address.
    pub async fn balance(
        &self,
        address: H160,
        at: &BlockNumberOrTagOrHash,
    ) -> Result<U256, ClientError> {
        let account_id = self.account_id(&address);
        let query = subxt_client::storage().system().account(account_id);
        let Some(account) = self.storage_api(at).await?.fetch(&query).await? else {
            return Ok(U256::zero());
        };

        let native = account.data.free.into();
        Ok(self.inner.native_to_evm_decimals(native))
    }

    /// Helper function to dry run a transaction.
    async fn do_dry_run(
        runtime_api: &subxt::runtime_api::RuntimeApi<SrcChainConfig, OnlineClient<SrcChainConfig>>,
        tx: &GenericTransaction,
    ) -> Result<
        crate::subxt_client::src_chain::runtime_types::eth_rpc_api::adapters::DryRunInfo<Balance>,
        ClientError,
    > {
        // TODO Remove encode/decode hack to convert to subxt generated type
        use crate::subxt_client::runtime_types::eth_rpc_api::rpc_types;
        let tx = rpc_types::GenericTransaction::decode(&mut tx.encode().as_slice()).unwrap();

        let payload = subxt_client::apis().contracts_evm_api().gas_estimate(tx);
        let result = runtime_api.call(payload).await?.map_err(|err| {
            log::debug!("Failed to dry_run: {err:?}");
            ClientError::DispatchError
        })?;

        Ok(result)
    }

    pub async fn get_contract_storage(
        &self,
        contract_address: H160,
        key: U256,
        block: BlockNumberOrTagOrHash,
    ) -> Result<Vec<u8>, ClientError> {
        let runtime_api = self.runtime_api(&block).await?;

        let account_id = self.account_id(&contract_address);
        let mut bytes = vec![0u8; 32];
        key.to_big_endian(&mut bytes);
        let payload = subxt_client::apis()
            .contracts_api()
            .get_storage(account_id, bytes);
        let result = runtime_api
            .call(payload)
            .await?
            .unwrap_or_default()
            .unwrap_or_default();
        Ok(result)
    }

    pub async fn get_contract_code(
        &self,
        contract_address: &H160,
        block: BlockNumberOrTagOrHash,
    ) -> Result<Vec<u8>, ClientError> {
        let storage_api = self.storage_api(&block).await?;
        let account_id = self.account_id(contract_address);
        let code_hash: H256 = account_id.0.into();
        let query = subxt_client::storage().contracts().pristine_code(code_hash);
        let result = storage_api
            .fetch(&query)
            .await?
            .map(|v| v.0)
            .unwrap_or_default();
        Ok(result)
    }

    /// Dry run a transaction and returns the [`DryRunInfo`] for the transaction.
    pub async fn dry_run(
        &self,
        tx: &GenericTransaction,
        block: BlockNumberOrTagOrHash,
    ) -> Result<DryRunInfo<Balance>, ClientError> {
        let runtime_api = self.runtime_api(&block).await?;
        let result = Self::do_dry_run(&runtime_api, tx).await?;

        Ok(DryRunInfo {
            gas_limit: Weight::from_parts(result.gas_limit.ref_time, result.gas_limit.proof_size),
            storage_deposit_limit: result.storage_deposit_limit,
            return_data: result.return_data,
        })
    }

    /// Dry run a transaction and returns the gas estimate for the transaction.
    pub async fn gas_estimate(
        &self,
        tx: &GenericTransaction,
        block: BlockNumberOrTagOrHash,
    ) -> Result<eth_rpc_api::U256, ClientError> {
        let runtime_api = self.runtime_api(&block).await?;
        let result = Self::do_dry_run(&runtime_api, tx).await?;
        let fee = Self::weight_to_fee(
            &runtime_api,
            Weight::from_parts(result.gas_limit.ref_time, result.gas_limit.proof_size),
        )
        .await?;

        let total_fee = fee + result.storage_deposit_limit.unwrap_or_default();
        Ok(U256::from(total_fee / GAS_PRICE) + 1)
    }

    /// Get the nonce of the given address.
    pub async fn nonce(
        &self,
        address: H160,
        block: BlockNumberOrTagOrHash,
    ) -> Result<u32, ClientError> {
        let account_id = self.account_id(&address);
        let storage = self.storage_api(&block).await?;
        let query = subxt_client::storage().system().account(account_id);
        let Some(account) = storage.fetch(&query).await? else {
            return Ok(0);
        };

        Ok(account.nonce)
    }

    /// Get the block number of the latest block.
    pub async fn block_number(&self) -> Result<SubstrateBlockNumber, ClientError> {
        let cache = self.inner.cache.read().await;
        let latest_block = cache.buffer.back().ok_or(ClientError::CacheEmpty)?;
        Ok(latest_block.number())
    }

    /// Get a block hash for the given block number.
    pub async fn get_block_hash(
        &self,
        block_number: SubstrateBlockNumber,
    ) -> Result<Option<SubstrateBlockHash>, ClientError> {
        let cache = self.inner.cache.read().await;
        if let Some(block) = cache.blocks_by_number.get(&block_number) {
            return Ok(Some(block.hash()));
        }

        let hash = self
            .inner
            .rpc
            .chain_get_block_hash(Some(block_number.into()))
            .await?;
        Ok(hash)
    }

    /// Get a block for the specified hash or number.
    pub async fn block_by_number_or_tag(
        &self,
        block: &BlockNumberOrTag,
    ) -> Result<Option<Arc<SubstrateBlock>>, ClientError> {
        match block {
            BlockNumberOrTag::U256(n) => {
                let n = (*n).try_into().map_err(|_| ClientError::ConversionFailed)?;
                self.block_by_number(n).await
            }
            BlockNumberOrTag::BlockTag(_) => {
                let cache = self.inner.cache.read().await;
                Ok(cache.buffer.back().cloned())
            }
        }
    }

    /// Get a block by hash
    pub async fn block_by_hash(
        &self,
        hash: &SubstrateBlockHash,
    ) -> Result<Option<Arc<SubstrateBlock>>, ClientError> {
        let cache = self.inner.cache.read().await;
        if let Some(block) = cache.blocks_by_hash.get(hash) {
            return Ok(Some(block.clone()));
        }

        match self.inner.api.blocks().at(*hash).await {
            Ok(block) => Ok(Some(Arc::new(block))),
            Err(subxt::Error::Block(subxt::error::BlockError::NotFound(_))) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    /// Get a block by number
    pub async fn block_by_number(
        &self,
        block_number: SubstrateBlockNumber,
    ) -> Result<Option<Arc<SubstrateBlock>>, ClientError> {
        let cache = self.inner.cache.read().await;
        if let Some(block) = cache.blocks_by_number.get(&block_number) {
            return Ok(Some(block.clone()));
        }

        let Some(hash) = self.get_block_hash(block_number).await? else {
            return Ok(None);
        };

        self.block_by_hash(&hash).await
    }

    /// Get the EVM block for the given hash.
    pub async fn evm_block(
        &self,
        block: Arc<SubstrateBlock>,
    ) -> Result<eth_rpc_api::Block, ClientError> {
        let runtime_api = self.inner.api.runtime_api().at(block.hash());
        let max_fee = Self::weight_to_fee(&runtime_api, self.max_block_weight()).await?;
        let gas_limit = U256::from(max_fee / GAS_PRICE);

        let header = block.header();
        let timestamp = extract_block_timestamp(&block).await.unwrap_or_default();
        Ok(eth_rpc_api::Block {
            hash: block.hash(),
            parent_hash: header.parent_hash,
            state_root: header.state_root,
            transactions_root: header.extrinsics_root,
            number: header.number.into(),
            timestamp: timestamp.into(),
            gas_limit,
            logs_bloom: Bytes256([0u8; 256]),
            receipts_root: header.extrinsics_root,
            ..Default::default()
        })
    }

    /// Convert a weight to a fee.
    async fn weight_to_fee(
        runtime_api: &subxt::runtime_api::RuntimeApi<SrcChainConfig, OnlineClient<SrcChainConfig>>,
        weight: Weight,
    ) -> Result<Balance, ClientError> {
        use crate::subxt_client::runtime_apis::transaction_payment_api::types::query_weight_to_fee;
        let payload = subxt_client::apis()
            .transaction_payment_api()
            .query_weight_to_fee(query_weight_to_fee::Weight {
                ref_time: weight.ref_time(),
                proof_size: weight.proof_size(),
            });

        let fee = runtime_api.call(payload).await?;
        Ok(fee)
    }

    /// Get the substrate account ID from the EVM address.
    pub fn account_id(&self, address: &H160) -> AccountId32 {
        let account_id = primitives::get_account_id(address);
        AccountId32(account_id.into())
    }

    /// Get the chain ID.
    pub fn chain_id(&self) -> u64 {
        self.inner.chain_id
    }

    /// Get the Max Block Weight.
    pub fn max_block_weight(&self) -> Weight {
        self.inner.max_block_weight
    }
}
