//! taken from https://github.com/anza-xyz/wallet-adapter/blob/master/packages/core/base/src/adapter.ts

use anyhow::Result;
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_sdk::transaction::Transaction;
use wallet_adapter_web3::SendTransactionOptions;
use wallet_adapter_web3::{Connection, SendOptions};

use crate::transaction::{SupportedTransactionVersions, TransactionOrVersionedTransaction};
use crate::WalletError;

#[derive(Debug)]
pub enum WalletAdapterEvent {
    Connect(Pubkey),
    Disconnect,
    Error(WalletError),
    ReadyStateChange(WalletReadyState),
}

#[derive(Debug, Clone)]
pub struct WalletAdapterEventEmitter {
    tx: tokio::sync::mpsc::Sender<WalletAdapterEvent>,
    rx: std::sync::Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<WalletAdapterEvent>>>,
}

impl WalletAdapterEventEmitter {
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        Self {
            tx,
            rx: std::sync::Arc::new(tokio::sync::Mutex::new(rx)),
        }
    }

    pub async fn emit(&self, event: WalletAdapterEvent) -> Result<()> {
        Ok(self.tx.send(event).await?)
    }
    pub fn emit_sync(&self, event: WalletAdapterEvent) -> Result<()> {
        Ok(self.tx.blocking_send(event)?)
    }

    pub async fn recv(&self) -> Option<WalletAdapterEvent> {
        self.rx.lock().await.recv().await
    }
}

/**
 * A wallet's readiness describes a series of states that the wallet can be in,
 * depending on what kind of wallet it is. An installable wallet (eg. a browser
 * extension like Phantom) might be `Installed` if we've found the Phantom API
 * in the global scope, or `NotDetected` otherwise. A loadable, zero-install
 * runtime (eg. Torus Wallet) might simply signal that it's `Loadable`. Use this
 * metadata to personalize the wallet list for each user (eg. to show their
 * installed wallets first).
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display)]
pub enum WalletReadyState {
    /**
     * User-installable wallets can typically be detected by scanning for an API
     * that they've injected into the global context. If such an API is present,
     * we consider the wallet to have been installed.
     */
    Installed,
    NotDetected,
    /**
     * Loadable wallets are always available to you. Since you can load them at
     * any time, it's meaningless to say that they have been detected.
     */
    Loadable,
    /**
     * If a wallet is not supported on a given platform (eg. server-rendering, or
     * mobile) then it will stay in the `Unsupported` state.
     */
    Unsupported,
}

#[async_trait::async_trait(?Send)]
pub trait BaseWalletAdapter {
    fn event_emitter(&self) -> WalletAdapterEventEmitter;
    fn name(&self) -> String;
    fn url(&self) -> String;
    fn icon(&self) -> String;
    fn ready_state(&self) -> WalletReadyState;
    fn public_key(&self) -> Option<Pubkey>;
    fn connecting(&self) -> bool;
    fn supported_transaction_versions(&self) -> Option<SupportedTransactionVersions>;

    fn connected(&self) -> bool {
        self.public_key().is_some()
    }

    async fn disconnect(&self) -> Result<()>;
    async fn auto_connect(&mut self) -> crate::Result<()> {
        self.connect().await
    }

    async fn connect(&mut self) -> crate::Result<()>;

    async fn send_transaction(
        &self,
        transaction: TransactionOrVersionedTransaction,
        connection: &dyn Connection,
        options: Option<SendTransactionOptions>,
    ) -> crate::Result<Signature>;

    async fn prepare_transaction(
        &self,
        mut transaction: Transaction,
        connection: &dyn Connection,
        options: Option<SendOptions>,
    ) -> crate::Result<Transaction> {
        let Some(public_key) = self.public_key() else {
            return Err(crate::WalletError::WalletNotConnected);
        };

        // TODO: double check if this works and makes sense
        if transaction.message.signer_keys().is_empty() {
            transaction.message.account_keys.push(public_key);
        }

        if transaction.message.recent_blockhash == Hash::default() {
            let blockhash = connection
                .get_recent_blockhash(
                    options.map(|o| o.preflight_commitment).flatten(),
                    options.map(|o| o.min_context_slots).flatten(),
                )
                .await?;
            transaction.message.recent_blockhash = blockhash;
        }

        Ok(transaction)
    }

    /// Check if the transaction is supported by the wallet
    fn check_if_transaction_is_supported(
        &self,
        transaction: &TransactionOrVersionedTransaction,
    ) -> crate::Result<()> {
        if let TransactionOrVersionedTransaction::VersionedTransaction(tx) = transaction {
            match self.supported_transaction_versions() {
                Some(versions) => {
                    if !versions.contains(&tx.version()) {
                        return Err(crate::WalletError::WalletSendTransactionError(format!(
                            "Sending transaction version {:?} isn't supported by this wallet",
                            tx.version()
                        )));
                    }
                }
                None => {
                    return Err(crate::WalletError::WalletSendTransactionError(
                        "Sending versioned transactions isn't supported by this wallet".to_string(),
                    ))
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_base_wallet_adapter_trait_be_made_into_object() {
        let _wallet_adapter: Option<Box<dyn BaseWalletAdapter>> = None;
    }
}
