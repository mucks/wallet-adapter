use serde::{Deserialize, Serialize};
use solana_sdk::transaction::TransactionVersion;
use wallet_adapter_web3::{Transaction, VersionedTransaction};

pub type SupportedTransactionVersions = Vec<TransactionVersion>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TransactionOrVersionedTransaction {
    Transaction(Transaction),
    VersionedTransaction(VersionedTransaction),
}

impl TransactionOrVersionedTransaction {
    pub fn is_versioned(&self) -> bool {
        matches!(self, Self::VersionedTransaction(_))
    }
}
