use solana_sdk::{signature::Signature, signer::Signer};
use wallet_adapter_common::{connection::Connection, types::SendTransactionOptions};

use crate::{adapter::BaseWalletAdapter, transaction::TransactionOrVersionedTransaction};
use anyhow::anyhow;

#[async_trait::async_trait(?Send)]
pub trait BaseSignerWalletAdapter: BaseWalletAdapter {
    fn wallet_signer(&self) -> Option<Box<dyn Signer>>;

    async fn send_transaction(
        &self,
        transaction: TransactionOrVersionedTransaction,
        connection: &dyn Connection,
        options: Option<SendTransactionOptions>,
    ) -> crate::Result<Signature> {
        if self.wallet_signer().is_none()
            && options
                .as_ref()
                .map(|o| o.signers.is_empty())
                .unwrap_or(true)
        {
            return Err(anyhow!("No signers available").into());
        }

        match transaction {
            TransactionOrVersionedTransaction::Transaction(tx) => {
                let mut signers: Vec<&dyn Signer> = vec![];

                let opt_wallet_signer = self.wallet_signer();
                if let Some(wallet_signer) = opt_wallet_signer.as_ref() {
                    signers.push(wallet_signer.as_ref());
                }

                let send_options = options.as_ref().map(|o| o.send_options);

                if let Some(ref options) = options {
                    signers.extend(options.signers.iter().map(|s| s.as_ref()));
                }

                let mut tx = self
                    .prepare_transaction(tx, connection, send_options.as_ref())
                    .await?;

                tx.partial_sign(&signers, tx.message.recent_blockhash);

                let tx = self
                    .sign_transaction(TransactionOrVersionedTransaction::Transaction(tx))
                    .await?;

                let TransactionOrVersionedTransaction::Transaction(tx) = tx else {
                    return Err(crate::WalletError::WalletSendTransactionError(
                        "Expected Transaction".to_string(),
                    ));
                };

                let raw_tx = bincode::serialize(&tx)?;

                return Ok(connection
                    .send_raw_transaction(raw_tx, options.as_ref())
                    .await?);
            }
            TransactionOrVersionedTransaction::VersionedTransaction(ref _tx) => {
                self.check_if_transaction_is_supported(&transaction)?;

                let tx = self.sign_transaction(transaction).await?;

                let TransactionOrVersionedTransaction::VersionedTransaction(tx) = tx else {
                    return Err(crate::WalletError::WalletSendTransactionError(
                        "Expected VersionedTransaction".to_string(),
                    ));
                };

                let raw_tx = bincode::serialize(&tx)?;

                return Ok(connection
                    .send_raw_transaction(raw_tx, options.as_ref())
                    .await?);
            }
        }
    }

    async fn sign_transaction(
        &self,
        transaction: TransactionOrVersionedTransaction,
    ) -> crate::Result<TransactionOrVersionedTransaction>;

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

#[async_trait::async_trait(?Send)]
pub trait BaseMessageSignerWalletAdapter: BaseSignerWalletAdapter {
    async fn sign_message(&self, message: &[u8]) -> crate::Result<Vec<u8>>;
}
