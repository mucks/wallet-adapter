//! types that the solana wallet adapter uses
//! `solana-sdk` doesn't have all the types the `web3.js` has so we need to define our own

use anyhow::Result;
use serde::{Deserialize, Serialize};
use solana_sdk::{
    commitment_config::CommitmentLevel, hash::Hash, pubkey::Pubkey, signature::Signature,
};

#[async_trait::async_trait]
pub trait Connection: Send + Sync {
    async fn get_recent_blockhash(
        &self,
        commitment: Option<CommitmentLevel>,
        min_context_slots: Option<u32>,
    ) -> Result<Hash>;

    async fn send_raw_transaction(
        &self,
        raw_transaction: Vec<u8>,
        options: Option<SendTransactionOptions>,
    ) -> Result<Signature>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendOptions {
    /** disable transaction verification step */
    pub skip_preflight: Option<bool>,
    /** preflight commitment level */
    pub preflight_commitment: Option<CommitmentLevel>,
    /** Maximum number of times for the RPC node to retry sending the transaction to the leader. */
    pub max_retries: Option<u32>,
    /** The minimum slot that the request can be evaluated at */
    pub min_context_slots: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendTransactionOptions {
    pub signers: Vec<Signer>,
    #[serde(flatten)]
    pub send_options: SendOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signer {}

impl solana_sdk::signature::Signer for Signer {
    fn try_pubkey(&self) -> std::result::Result<Pubkey, solana_sdk::signer::SignerError> {
        todo!()
    }

    fn try_sign_message(
        &self,
        message: &[u8],
    ) -> std::result::Result<Signature, solana_sdk::signer::SignerError> {
        todo!()
    }

    fn is_interactive(&self) -> bool {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    fee_payer: Option<Pubkey>,
    recent_block_hash: Option<Hash>,
    pub sdk_transaction: solana_sdk::transaction::Transaction,
}

impl Transaction {
    pub fn new(sdk_transaction: solana_sdk::transaction::Transaction) -> Self {
        Self {
            fee_payer: None,
            recent_block_hash: None,
            sdk_transaction,
        }
    }

    pub fn set_fee_payer(&mut self, fee_payer: Pubkey) {
        self.fee_payer = Some(fee_payer);
    }

    pub fn fee_payer(&self) -> Option<Pubkey> {
        self.fee_payer
    }

    pub fn recent_block_hash(&self) -> Option<Hash> {
        self.recent_block_hash
    }

    pub fn set_recent_block_hash(&mut self, recent_block_hash: Hash) {
        self.recent_block_hash = Some(recent_block_hash);
    }
}
