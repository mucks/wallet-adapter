use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use solana_sdk::{pubkey::Pubkey, transaction::TransactionVersion};
use wallet_adapter_base::{
    BaseWalletAdapter, SupportedTransactionVersions, TransactionOrVersionedTransaction,
    WalletAdapterEvent, WalletAdapterEventEmitter, WalletError, WalletReadyState,
};
use wallet_adapter_common::connection::Connection;
use wallet_adapter_common::types::SendTransactionOptions;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{prelude::Closure, JsCast};

mod wallet_binding {
    use super::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen]
        #[derive(Clone, Debug)]
        pub type Pubkey;

        #[wasm_bindgen(method, js_name = toBytes)]
        pub fn to_bytes(this: &Pubkey) -> Vec<u8>;

    }
}

#[async_trait::async_trait(?Send)]
pub trait GenericWasmWallet: Sync + Send + std::fmt::Debug + Clone {
    fn is_correct_wallet(&self) -> bool;
    fn is_connected(&self) -> bool;
    async fn connect(&self) -> Result<()>;
    fn disconnect(&self) -> Result<()>;
    async fn sign_and_send_transaction(
        &self,
        transaction: TransactionOrVersionedTransaction,
    ) -> Result<solana_sdk::signature::Signature>;
    fn on(&self, event: &str, cb: js_sys::Function) -> Result<()>;
    fn off(&self, event: &str, cb: js_sys::Function) -> Result<()>;
    fn public_key(&self) -> Result<Pubkey>;
    fn name(&self) -> String;
    fn url(&self) -> String;
    fn icon(&self) -> String;
    fn is_ios_redirectable(&self) -> Result<bool> {
        Ok(false)
    }
    fn set_wallet_url(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct GenericWasmWalletAdapter<T: GenericWasmWallet + 'static> {
    connecting: Arc<Mutex<bool>>,
    wallet: Arc<T>,
    public_key: Arc<Mutex<Option<Pubkey>>>,
    wallet_ready_state: Arc<Mutex<WalletReadyState>>,
    account_changed_closure: Arc<Mutex<Option<Closure<dyn FnMut(wallet_binding::Pubkey)>>>>,
    disconnected_closure: Arc<Mutex<Option<Closure<dyn FnMut()>>>>,
    event_emitter: WalletAdapterEventEmitter,
}

impl<T: GenericWasmWallet + 'static> GenericWasmWalletAdapter<T> {
    pub fn new(wallet: T) -> Result<Self> {
        let adapter = Self {
            event_emitter: WalletAdapterEventEmitter::new(),
            connecting: Arc::new(Mutex::new(false)),
            wallet: Arc::new(wallet),
            public_key: Arc::new(Mutex::new(None)),
            wallet_ready_state: Arc::new(Mutex::new(WalletReadyState::NotDetected)),
            account_changed_closure: Arc::new(Mutex::new(None)),
            disconnected_closure: Arc::new(Mutex::new(None)),
        };

        if adapter.ready_state() != WalletReadyState::Unsupported {
            if adapter.wallet.is_ios_redirectable()? {
                *adapter.wallet_ready_state.lock().unwrap() = WalletReadyState::Loadable;
                // js lib emits event here
            } else {
                let self_clone = adapter.clone();

                // TODO: make this waiting loop a shared logic
                wasm_bindgen_futures::spawn_local(async move {
                    for _i in 0..60 {
                        if self_clone.wallet.is_correct_wallet() {
                            tracing::debug!("wallet detected {}", self_clone.wallet.name());
                            self_clone.set_ready_state(WalletReadyState::Installed);
                            self_clone
                                .event_emitter
                                .emit(WalletAdapterEvent::ReadyStateChange(
                                    WalletReadyState::Installed,
                                ))
                                .await
                                .unwrap();
                            break;
                        }
                        crate::util::sleep_ms(1000).await;
                    }
                });
            }
        }

        Ok(adapter)
    }

    fn disconnected(&self) -> js_sys::Function {
        let mut disconnected = self.disconnected_closure.lock().unwrap();

        if let Some(closure) = disconnected.as_ref() {
            let f: &js_sys::Function = closure.as_ref().unchecked_ref();
            return f.clone();
        } else {
            let closure = Closure::wrap(Box::new(move || {
                // disconnected code here
                tracing::info!("disconnected");
            }) as Box<dyn FnMut()>);
            let f: &js_sys::Function = closure.as_ref().unchecked_ref();

            let f = f.clone();
            *disconnected = Some(closure);

            f
        }
    }

    fn account_changed(&self) -> js_sys::Function {
        let mut account_changed = self.account_changed_closure.lock().unwrap();

        if let Some(closure) = account_changed.as_ref() {
            let f: &js_sys::Function = closure.as_ref().unchecked_ref();
            return f.clone();
        } else {
            let self_clone = self.clone();
            let closure = Closure::wrap(Box::new(move |pubkey: wallet_binding::Pubkey| {
                // disconnected code here
                tracing::info!("account changed: {pubkey:?}");

                let public_key: Pubkey = pubkey.to_bytes().try_into().unwrap();

                if self_clone.public_key() == Some(public_key) {
                    return;
                }

                self_clone.set_public_key(Some(public_key));
                self_clone
                    .event_emitter
                    .emit_sync(WalletAdapterEvent::Connect(public_key))
                    .unwrap();
            }) as Box<dyn FnMut(wallet_binding::Pubkey)>);
            let f: &js_sys::Function = closure.as_ref().unchecked_ref();
            let f = f.clone();
            *account_changed = Some(closure);
            f
        }
    }

    fn set_connecting(&self, connecting: bool) {
        *self.connecting.lock().unwrap() = connecting;
    }

    fn set_public_key(&self, public_key: Option<Pubkey>) {
        *self.public_key.lock().unwrap() = public_key;
    }

    fn set_ready_state(&self, ready_state: WalletReadyState) {
        *self.wallet_ready_state.lock().unwrap() = ready_state;
    }

    async fn try_connect(&mut self) -> wallet_adapter_base::Result<()> {
        tracing::info!("{} connect", self.name());

        if self.connected() || self.connecting() {
            return Ok(());
        }

        if self.ready_state() == WalletReadyState::Loadable {
            self.wallet.set_wallet_url()?;
        }

        if self.ready_state() != WalletReadyState::Installed {
            return Err(wallet_adapter_base::WalletError::WalletNotReady);
        }

        self.set_connecting(true);

        if !self.wallet.is_connected() {
            match self.wallet.connect().await {
                Ok(_) => {}
                Err(e) => {
                    return Err(e.into());
                }
            }
        }

        let public_key = self.wallet.public_key()?;

        self.wallet.on("disconnect", self.disconnected())?;
        self.wallet.on("accountChanged", self.account_changed())?;

        self.set_public_key(Some(public_key));

        self.event_emitter
            .emit(WalletAdapterEvent::Connect(public_key))
            .await?;

        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl<T: GenericWasmWallet + 'static> BaseWalletAdapter for GenericWasmWalletAdapter<T> {
    fn event_emitter(&self) -> WalletAdapterEventEmitter {
        self.event_emitter.clone()
    }

    fn name(&self) -> String {
        self.wallet.name()
    }

    fn url(&self) -> String {
        self.wallet.url()
    }

    fn icon(&self) -> String {
        self.wallet.icon()
    }

    fn connected(&self) -> bool {
        self.public_key.lock().unwrap().is_some()
    }

    fn ready_state(&self) -> WalletReadyState {
        self.wallet_ready_state.lock().unwrap().clone()
    }

    fn public_key(&self) -> Option<Pubkey> {
        self.public_key.lock().unwrap().clone()
    }

    fn connecting(&self) -> bool {
        self.connecting.lock().unwrap().clone()
    }

    fn supported_transaction_versions(&self) -> Option<SupportedTransactionVersions> {
        Some(vec![
            TransactionVersion::LEGACY,
            TransactionVersion::Number(0),
        ])
    }

    async fn auto_connect(&mut self) -> wallet_adapter_base::Result<()> {
        if self.ready_state() == WalletReadyState::Installed {
            return self.connect().await;
        }
        Ok(())
    }

    async fn connect(&mut self) -> wallet_adapter_base::Result<()> {
        if let Err(err) = self.try_connect().await {
            self.event_emitter
                .emit(WalletAdapterEvent::Error(err))
                .await?
        }

        self.set_connecting(false);

        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.wallet.off("disconnect", self.disconnected())?;
        self.wallet.off("accountChanged", self.account_changed())?;

        self.set_public_key(None);

        if let Err(err) = self.wallet.disconnect() {
            self.event_emitter
                .emit(WalletAdapterEvent::Error(WalletError::Anyhow(err.into())))
                .await?;
        }

        self.event_emitter
            .emit(WalletAdapterEvent::Disconnect)
            .await?;

        Ok(())
    }

    async fn send_transaction(
        &self,
        mut transaction: wallet_adapter_base::TransactionOrVersionedTransaction,
        connection: &dyn Connection,
        options: Option<SendTransactionOptions>,
    ) -> wallet_adapter_base::Result<solana_sdk::signature::Signature> {
        if self.public_key().is_none() {
            return Err(WalletError::WalletNotConnected);
        }

        let send_options = options.as_ref().map(|o| o.send_options);

        match &mut transaction {
            TransactionOrVersionedTransaction::Transaction(ref mut tx) => {
                *tx = self
                    .prepare_transaction(tx.clone(), connection, send_options.as_ref())
                    .await?;

                if let Some(opt) = options {
                    if opt.signers.len() > 0 {
                        tx.partial_sign(&opt.signers, tx.message.recent_blockhash);
                    }
                }
            }
            TransactionOrVersionedTransaction::VersionedTransaction(ref mut tx) => {
                if let Some(opt) = options {
                    if opt.signers.len() > 0 {
                        // TODO: implement support for VersionedTransaction
                        return Err(
                            anyhow!("Unsupported transaction version: {:?}", tx.version()).into(),
                        );
                    }
                }
            }
        }

        Ok(self.wallet.sign_and_send_transaction(transaction).await?)
    }
}
