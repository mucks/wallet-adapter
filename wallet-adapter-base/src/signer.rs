use serde::Serialize;
use solana_sdk::{
    signature::Signature,
    transaction::{self, VersionedTransaction},
};
use wallet_adapter_web3::{Connection, SendTransactionOptions};

use crate::{adapter::BaseWalletAdapter, transaction::TransactionOrVersionedTransaction};
use anyhow::{Context, Result};

pub trait BaseSignerWalletAdapter: BaseWalletAdapter {
    async fn send_transaction(
        &self,
        transaction: TransactionOrVersionedTransaction,
        connection: impl Connection,
        options: Option<SendTransactionOptions>,
    ) -> crate::Result<Signature> {
        match transaction {
            TransactionOrVersionedTransaction::Transaction(tx) => {
                let SendTransactionOptions {
                    signers,
                    send_options,
                } = options
                    .clone()
                    .context("Signers are required for transaction")?;

                let mut tx = self
                    .prepare_transaction(tx, &connection, Some(send_options))
                    .await?;

                tx.sdk_transaction
                    .partial_sign(&signers, tx.recent_block_hash().unwrap());

                let tx = self
                    .sign_transaction(TransactionOrVersionedTransaction::Transaction(tx))
                    .await?;

                let raw_tx = bincode::serialize(&tx)?;

                return Ok(connection.send_raw_transaction(raw_tx, options).await?);
            }
            TransactionOrVersionedTransaction::VersionedTransaction(ref tx) => {
                self.check_if_transaction_is_supported(&transaction)?;

                let tx = self.sign_transaction(transaction).await?;
                let raw_tx = bincode::serialize(&tx)?;

                return Ok(connection.send_raw_transaction(raw_tx, options).await?);
            }
        }
    }

    async fn sign_transaction(
        &self,
        transaction: TransactionOrVersionedTransaction,
    ) -> Result<TransactionOrVersionedTransaction>;

    async fn sign_all_transactions(
        &self,
        transactions: Vec<TransactionOrVersionedTransaction>,
    ) -> crate::Result<Vec<TransactionOrVersionedTransaction>> {
        for transaction in transactions.iter() {
            self.check_if_transaction_is_supported(transaction)?;
        }

        let mut signed_transactions = Vec::new();
        for transaction in transactions {
            signed_transactions.push(self.sign_transaction(transaction).await?);
        }
        Ok(signed_transactions)
    }
}

#[async_trait::async_trait]
pub trait BaseMessageSignerWalletAdapter: BaseSignerWalletAdapter {
    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>>;
}
