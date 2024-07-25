//! Unchecked extrinsic with support for Ethereum signatures.
use crate::{MultiAddress, MultiSignature};
use eth_rpc_api::{SignerRecovery, TransactionLegacyUnsigned, TransactionUnsigned};
#[cfg(not(feature = "std"))]
use frame::deps::sp_std::alloc::format;
use frame::{
    deps::{
        frame_support::dispatch::{DispatchInfo, GetDispatchInfo},
        sp_core,
        sp_runtime::{
            generic::{self, CheckedExtrinsic},
            traits::{
                self, Checkable, Convert, Extrinsic, ExtrinsicMetadata, Member, SignedExtension,
            },
            transaction_validity::{InvalidTransaction, TransactionValidityError},
            AccountId32, RuntimeDebug,
        },
    },
    log,
    prelude::*,
    primitives::H160,
    traits::ExtrinsicCall,
};

/// Unchecked extrinsic with support for Ethereum signatures.
/// This is a wrapper on top of [`generic::UncheckedExtrinsic`] to support Ethereum
/// transactions.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(ConvertEthTx))]
pub struct UncheckedExtrinsic<Call, Extra: SignedExtension, ConvertEthTx>(
    pub generic::UncheckedExtrinsic<MultiAddress, Call, MultiSignature, Extra>,
    PhantomData<ConvertEthTx>,
);

impl<Call: TypeInfo, Extra: SignedExtension, ConvertEthTx> Extrinsic
    for UncheckedExtrinsic<Call, Extra, ConvertEthTx>
{
    type Call = Call;

    type SignaturePayload = (MultiAddress, MultiSignature, Extra);

    fn is_signed(&self) -> Option<bool> {
        self.0.is_signed()
    }

    fn new(function: Call, signed_data: Option<Self::SignaturePayload>) -> Option<Self> {
        Some(if let Some((address, signature, extra)) = signed_data {
            Self(
                generic::UncheckedExtrinsic::new_signed(function, address, signature, extra),
                PhantomData,
            )
        } else {
            Self(
                generic::UncheckedExtrinsic::new_unsigned(function),
                PhantomData,
            )
        })
    }
}

impl<Call, Extra: SignedExtension, ConvertEthTx> ExtrinsicMetadata
    for UncheckedExtrinsic<Call, Extra, ConvertEthTx>
{
    const VERSION: u8 =
        generic::UncheckedExtrinsic::<MultiAddress, Call, MultiSignature, Extra>::VERSION;
    type SignedExtensions = Extra;
}

impl<Call: TypeInfo, Extra: SignedExtension, ConvertEthTx> ExtrinsicCall
    for UncheckedExtrinsic<Call, Extra, ConvertEthTx>
{
    fn call(&self) -> &Self::Call {
        self.0.call()
    }
}

impl<Call, Extra, ConvertEthTx, Lookup> Checkable<Lookup>
    for UncheckedExtrinsic<Call, Extra, ConvertEthTx>
where
    Call: Encode + Member,
    Extra: SignedExtension<AccountId = AccountId32>,
    ConvertEthTx:
        Convert<(Call, Extra), Result<(TransactionUnsigned, H160, Extra), InvalidTransaction>>,
    Lookup: traits::Lookup<Source = MultiAddress, Target = AccountId32>,
{
    type Checked = CheckedExtrinsic<AccountId32, Call, Extra>;

    fn check(self, lookup: &Lookup) -> Result<Self::Checked, TransactionValidityError> {
        let function = self.0.function.clone();

        match self.0.signature {
            Some((addr, MultiSignature::Ethereum(sig), extra)) => {
                log::trace!(target: "evm", "Checking extrinsic with  ethereum signature...");
                let (eth_msg, source, eth_extra) =
                    ConvertEthTx::convert((function.clone(), extra))?;

                let msg = TransactionLegacyUnsigned::try_from(eth_msg)
                    .map_err(|_| InvalidTransaction::Call)?;

                log::trace!(target: "evm", "Received ethereum transaction: {msg:#?}");

                let signer = msg
                    .recover_signer(&sig)
                    .ok_or(InvalidTransaction::BadProof)?;
                if signer != source {
                    log::trace!(target: "evm", "Invalid recovered signer: ({signer:?}) != source ({source:?}");
                    return Err(InvalidTransaction::BadProof.into());
                }

                let account_id = lookup.lookup(MultiAddress::Address20(signer.into()))?;
                log::trace!(target: "evm", "Signer address20 is: {account_id:?}");

                let expected_account_id = lookup.lookup(addr)?;

                if account_id != expected_account_id {
                    log::trace!(target: "evm", "Account ID should be: {expected_account_id:?}");
                    return Err(InvalidTransaction::BadProof.into());
                }

                log::trace!(target: "evm", "Valid ethereum message");
                Ok(CheckedExtrinsic {
                    signed: Some((account_id, eth_extra)),
                    function,
                })
            }
            _ => self.0.check(lookup),
        }
    }
}

impl<Call, Extra, ConvertEthTx> GetDispatchInfo for UncheckedExtrinsic<Call, Extra, ConvertEthTx>
where
    Call: GetDispatchInfo,
    Extra: SignedExtension,
{
    fn get_dispatch_info(&self) -> DispatchInfo {
        self.0.get_dispatch_info()
    }
}

impl<Call: Encode, Extra: SignedExtension, ConvertEthTx> serde::Serialize
    for UncheckedExtrinsic<Call, Extra, ConvertEthTx>
{
    fn serialize<S>(&self, seq: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        self.0.serialize(seq)
    }
}

impl<'a, Call: Decode, Extra: SignedExtension, ConvertEthTx> serde::Deserialize<'a>
    for UncheckedExtrinsic<Call, Extra, ConvertEthTx>
{
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let r = sp_core::bytes::deserialize(de)?;
        Decode::decode(&mut &r[..])
            .map_err(|e| serde::de::Error::custom(format!("Decode error: {}", e)))
    }
}
