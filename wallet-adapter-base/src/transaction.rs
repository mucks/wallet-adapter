use anyhow::Result;
use solana_sdk::transaction::{Transaction, TransactionVersion, VersionedTransaction};

pub type SupportedTransactionVersions = Vec<TransactionVersion>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionOrVersionedTransaction {
    Transaction(Transaction),
    VersionedTransaction(VersionedTransaction),
}

impl TransactionOrVersionedTransaction {
    pub fn is_versioned(&self) -> bool {
        matches!(self, Self::VersionedTransaction(_))
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        Ok(match self {
            Self::Transaction(tx) => bincode::serialize(&tx)?,
            Self::VersionedTransaction(tx) => bincode::serialize(&tx)?,
        })
    }
}
