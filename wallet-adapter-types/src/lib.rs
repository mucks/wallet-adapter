use serde::{Deserialize, Serialize};
use solana_sdk::{commitment_config::CommitmentLevel, signer::Signer};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendTransactionOptions {
    #[serde(skip)]
    pub signers: Vec<Box<dyn Signer>>,
    #[serde(flatten)]
    pub send_options: SendOptions,
}
