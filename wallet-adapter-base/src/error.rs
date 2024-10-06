pub type Result<T> = std::result::Result<T, WalletError>;

#[derive(Debug, strum::Display)]
pub enum WalletError {
    WalletNotReady,
    WalletLoad,
    WalletConfig,
    WalletConnection((String, String)),
    WalletDisconnected,
    WalletDisconnection((String, String)),
    WalletNotConnected,
    WalletSendTransactionError(String),
    BincodeSerializationError(bincode::Error),
    Anyhow(anyhow::Error),
}

impl From<anyhow::Error> for WalletError {
    fn from(e: anyhow::Error) -> Self {
        Self::Anyhow(e)
    }
}

impl From<bincode::Error> for WalletError {
    fn from(e: bincode::Error) -> Self {
        Self::BincodeSerializationError(e)
    }
}
