pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, strum::Display)]
pub enum Error {
    WalletNotReady,
    WalletLoad,
    WalletConfig,
    WalletConnection((String, String)),
    WalletDisconnected,
    WalletDisconnection,
    WalletNotConnected,
    WalletSendTransactionError(String),
    BincodeSerializationError(bincode::Error),
    Anyhow(anyhow::Error),
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Self::Anyhow(e)
    }
}

impl From<bincode::Error> for Error {
    fn from(e: bincode::Error) -> Self {
        Self::BincodeSerializationError(e)
    }
}
