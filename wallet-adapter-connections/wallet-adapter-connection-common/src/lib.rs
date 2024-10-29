use anyhow::Result;
use serde::{Deserialize, Serialize};
use solana_sdk::{commitment_config::CommitmentLevel, hash::Hash, signature::Signature};
use wallet_adapter_types::SendTransactionOptions;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub slot: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Value {
    pub blockhash: String,
    pub last_valid_block_height: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLatestBlockhash {
    pub context: Context,
    pub value: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcResponse<T, U> {
    pub jsonrpc: String,
    pub result: Option<T>,
    pub error: Option<U>,
    pub id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcRequest<T> {
    pub jsonrpc: String,
    pub method: String,
    pub params: T,
    pub id: u64,
}

impl<T> RpcRequest<T> {
    pub fn new(method: impl ToString, params: T) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: 1,
        }
    }
}
#[async_trait::async_trait(?Send)]
pub trait Connection {
    async fn get_recent_blockhash(
        &self,
        commitment: Option<CommitmentLevel>,
        min_context_slots: Option<u32>,
    ) -> Result<Hash>;

    async fn send_raw_transaction(
        &self,
        raw_transaction: Vec<u8>,
        options: Option<&SendTransactionOptions>,
    ) -> Result<Signature>;
}
