use std::str::FromStr;

use anyhow::{bail, Context, Result};
use base64::prelude::*;
use serde_json::json;
use solana_sdk::hash::Hash;
use solana_sdk::{commitment_config::CommitmentLevel, signature::Signature};
use wallet_adapter_common::connection::{Connection, GetLatestBlockhash, RpcRequest, RpcResponse};
use wallet_adapter_common::types::SendTransactionOptions;

pub struct WasmConnection {
    url: String,
}

impl WasmConnection {
    pub fn new(url: String) -> Self {
        Self { url }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn devnet() -> Self {
        Self::new("https://api.devnet.solana.com".to_string())
    }

    pub fn mainnet() -> Self {
        Self::new("https://api.mainnet-beta.solana.com".to_string())
    }

    pub fn testnet() -> Self {
        Self::new("https://api.testnet.solana.com".to_string())
    }
}

#[async_trait::async_trait(?Send)]
impl Connection for WasmConnection {
    async fn get_recent_blockhash(
        &self,
        commitment: Option<CommitmentLevel>,
        _min_context_slots: Option<u32>,
    ) -> Result<Hash> {
        let req = RpcRequest::new(
            "getLatestBlockhash",
            json!([{"commitment": commitment.unwrap_or(CommitmentLevel::Finalized)}]),
        );

        let client = reqwest::Client::new();

        let resp: RpcResponse<GetLatestBlockhash, serde_json::Value> = client
            .post(self.url())
            .json(&req)
            .header("Content-Type", "application/json")
            .send()
            .await?
            .json()
            .await?;

        tracing::debug!("resp: {}", serde_json::to_string_pretty(&resp)?);

        if let Some(err) = resp.error {
            bail!("Error: {}", serde_json::to_string_pretty(&err)?);
        }

        Ok(resp.result.context("no result")?.value.blockhash.parse()?)
    }

    async fn send_raw_transaction(
        &self,
        raw_transaction: Vec<u8>,
        options: Option<&SendTransactionOptions>,
    ) -> Result<Signature> {
        tracing::debug!("||| send_raw_transaction |||");

        let tx_base64 = BASE64_STANDARD.encode(&raw_transaction);

        let req_options = match options {
            Some(options) => json!({
                "skipPreflight": options.send_options.skip_preflight,
                "preflightCommitment": options.send_options.preflight_commitment,
                "maxRetries": options.send_options.max_retries,
                "minContextSlots": options.send_options.min_context_slots,
                "encoding": "base64"
            }),
            None => json!({
                "encoding": "base64"
            }),
        };

        let req = RpcRequest::new("sendTransaction", json!([tx_base64, req_options]));

        let client = reqwest::Client::new();

        let resp: RpcResponse<String, serde_json::Value> = client
            .post(self.url())
            .json(&req)
            .header("Content-Type", "application/json")
            .send()
            .await?
            .json()
            .await?;

        tracing::debug!("resp: {}", serde_json::to_string_pretty(&resp)?);

        if let Some(err) = resp.error {
            bail!("Error: {}", serde_json::to_string_pretty(&err)?);
        }

        Ok(Signature::from_str(&resp.result.context("no result")?)?)
    }
}
