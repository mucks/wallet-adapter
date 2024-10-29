use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use anyhow::Result;
use solana_sdk::{signature::Keypair, signer::Signer, transaction::TransactionVersion};
use wallet_adapter_base::{
    BaseMessageSignerWalletAdapter, BaseSignerWalletAdapter, BaseWalletAdapter, WalletAdapterEvent,
    WalletAdapterEventEmitter, WalletError, WalletReadyState,
};
use wallet_adapter_connection_common::Connection;
use wallet_adapter_types::SendTransactionOptions;

#[cfg(feature = "wasm")]
pub mod wasm_storage {
    use anyhow::{anyhow, Context, Result};
    use solana_sdk::signature::Keypair;
    use web_sys::Storage;

    use crate::KeypairStorage;

    #[derive(Debug)]
    pub struct WasmStorage {
        storage: Storage,
    }

    impl WasmStorage {
        pub fn local() -> Result<Self> {
            Ok(Self {
                storage: web_sys::window()
                    .context("window not available")?
                    .local_storage()
                    .map_err(|err| anyhow!("local storage not available: {err:?}"))?
                    .context("local storage not available")?,
            })
        }
    }

    impl KeypairStorage for WasmStorage {
        fn get_keypair(&self) -> Result<Option<Keypair>> {
            let item = self
                .storage
                .get_item("keypair")
                .map_err(|err| anyhow!("{err:?}"))?;
            match item {
                Some(item) => Ok(Some(Keypair::from_bytes(&hex::decode(item)?)?)),
                None => Ok(None),
            }
        }

        fn set_keypair(&self, keypair: Keypair) -> Result<()> {
            self.storage
                .set_item("keypair", &hex::encode(keypair.to_bytes()))
                .map_err(|err| anyhow!("{err:?}"))?;

            Ok(())
        }
    }
}

#[cfg(feature = "file-system")]
mod desktop_storage {}

pub trait KeypairStorage: std::fmt::Debug {
    fn get_keypair(&self) -> Result<Option<Keypair>>;
    fn set_keypair(&self, keypair: Keypair) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct UnsafePersistentWallet {
    /**
     * Storing a keypair locally like this is not safe because any application using this adapter could retrieve the
     * secret key
     */
    keypair: Arc<Mutex<Option<Keypair>>>,
    keypair_storage: Arc<Box<dyn KeypairStorage>>,
    event_emitter: WalletAdapterEventEmitter,
}

impl UnsafePersistentWallet {
    pub fn new(keypair_storage: impl KeypairStorage + 'static) -> Result<Self> {
        if keypair_storage.get_keypair()?.is_none() {
            keypair_storage.set_keypair(Keypair::new())?;
        }

        Ok(Self {
            keypair: Arc::new(Mutex::new(None)),
            keypair_storage: Arc::new(Box::new(keypair_storage)),
            event_emitter: WalletAdapterEventEmitter::new(),
        })
    }
}

#[async_trait::async_trait(?Send)]
impl BaseWalletAdapter for UnsafePersistentWallet {
    fn event_emitter(&self) -> wallet_adapter_base::WalletAdapterEventEmitter {
        self.event_emitter.clone()
    }

    fn name(&self) -> String {
        "UnsafePersistentWallet".to_string()
    }

    fn url(&self) -> String {
        "https://github.com/mucks/wallet-adapter".to_string()
    }

    fn icon(&self) -> String {
        "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMzQiIGhlaWdodD0iMzAiIGZpbGw9Im5vbmUiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZmlsbC1ydWxlPSJldmVub2RkIiBjbGlwLXJ1bGU9ImV2ZW5vZGQiIGQ9Ik0zNCAxMC42djIuN2wtOS41IDE2LjVoLTQuNmw2LTEwLjVhMi4xIDIuMSAwIDEgMCAyLTMuNGw0LjgtOC4zYTQgNCAwIDAgMSAxLjMgM1ptLTQuMyAxOS4xaC0uNmw0LjktOC40djQuMmMwIDIuMy0yIDQuMy00LjMgNC4zWm0yLTI4LjRjLS4zLS44LTEtMS4zLTItMS4zaC0xLjlsLTIuNCA0LjNIMzBsMS43LTNabS0zIDVoLTQuNkwxMC42IDI5LjhoNC43TDI4LjggNi40Wk0xOC43IDBoNC42bC0yLjUgNC4zaC00LjZMMTguNiAwWk0xNSA2LjRoNC42TDYgMjkuOEg0LjJjLS44IDAtMS43LS4zLTIuNC0uOEwxNSA2LjRaTTE0IDBIOS40TDcgNC4zaDQuNkwxNCAwWm0tMy42IDYuNEg1LjdMMCAxNi4ydjhMMTAuMyA2LjRaTTQuMyAwaC40TDAgOC4ydi00QzAgMiAxLjkgMCA0LjMgMFoiIGZpbGw9IiM5OTQ1RkYiLz48L3N2Zz4=".to_string()
    }

    fn ready_state(&self) -> WalletReadyState {
        WalletReadyState::Loadable
    }

    fn public_key(&self) -> Option<solana_sdk::pubkey::Pubkey> {
        self.keypair
            .lock()
            .ok()?
            .as_ref()
            .map(|keypair| keypair.pubkey())
    }

    fn connecting(&self) -> bool {
        false
    }

    fn supported_transaction_versions(
        &self,
    ) -> Option<wallet_adapter_base::SupportedTransactionVersions> {
        Some(vec![
            TransactionVersion::LEGACY,
            TransactionVersion::Number(0),
        ])
    }

    async fn connect(&mut self) -> wallet_adapter_base::Result<()> {
        let kp = self
            .keypair_storage
            .get_keypair()?
            .ok_or(WalletError::WalletNotReady)?;

        let public_key = kp.pubkey();
        *self.keypair.lock().map_err(|err| anyhow!("{err:?}"))? = Some(kp);

        self.event_emitter
            .emit(WalletAdapterEvent::Connect(public_key))
            .await?;

        Ok(())
    }

    async fn disconnect(&self) -> anyhow::Result<()> {
        *self.keypair.lock().map_err(|err| anyhow!("{err:?}"))? = None;
        self.event_emitter
            .emit(WalletAdapterEvent::Disconnect)
            .await?;

        Ok(())
    }

    async fn send_transaction(
        &self,
        transaction: wallet_adapter_base::TransactionOrVersionedTransaction,
        connection: &dyn Connection,
        options: Option<SendTransactionOptions>,
    ) -> wallet_adapter_base::Result<solana_sdk::signature::Signature> {
        <Self as BaseSignerWalletAdapter>::send_transaction(&self, transaction, connection, options)
            .await
    }
}

#[async_trait::async_trait(?Send)]
impl BaseSignerWalletAdapter for UnsafePersistentWallet {
    fn wallet_signer(&self) -> Option<Box<dyn Signer>> {
        let opt_kp = self.keypair.lock().ok().unwrap();
        let kp = opt_kp.as_ref()?;

        Some(Box::new(kp.insecure_clone()))
    }

    async fn sign_transaction(
        &self,
        mut transaction: wallet_adapter_base::TransactionOrVersionedTransaction,
    ) -> wallet_adapter_base::Result<wallet_adapter_base::TransactionOrVersionedTransaction> {
        let opt_kp = self.keypair.lock().map_err(|err| anyhow!("{err:?}"))?;
        let kp = opt_kp
            .as_ref()
            .ok_or_else(|| WalletError::WalletNotConnected)?;

        match transaction {
            wallet_adapter_base::TransactionOrVersionedTransaction::VersionedTransaction(
                ref mut vtx,
            ) => {
                // TODO: implement support for VersionedTransaction
                return Err(anyhow!("Unsupported transaction version: {:?}", vtx.version()).into());
            }
            wallet_adapter_base::TransactionOrVersionedTransaction::Transaction(ref mut tx) => {
                tx.partial_sign(&[kp], tx.message.recent_blockhash);
            }
        }

        Ok(transaction)
    }
}

#[async_trait::async_trait(?Send)]
impl BaseMessageSignerWalletAdapter for UnsafePersistentWallet {
    async fn sign_message(&self, message: &[u8]) -> wallet_adapter_base::Result<Vec<u8>> {
        let opt_kp = self.keypair.lock().map_err(|err| anyhow!("{err:?}"))?;
        let kp = opt_kp
            .as_ref()
            .ok_or_else(|| WalletError::WalletNotConnected)?;

        let sig_bytes: [u8; 64] = kp.sign_message(message).into();

        Ok(sig_bytes.to_vec())
    }
}
