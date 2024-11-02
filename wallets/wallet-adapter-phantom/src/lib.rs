use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::{bs58, pubkey::Pubkey, transaction::TransactionVersion};
use wallet_adapter_base::{
    BaseWalletAdapter, SupportedTransactionVersions, TransactionOrVersionedTransaction,
    WalletAdapterEvent, WalletAdapterEventEmitter, WalletError, WalletReadyState,
};
use wallet_adapter_common::connection::Connection;
use wallet_adapter_common::types::SendTransactionOptions;
use wallet_binding::solana;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};
use web_sys::Window;

mod wallet_binding {
    use super::*;

    // PhantomRequestResponse
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen]
        #[derive(Clone, Debug)]
        pub type PhantomRequestResponse;

        #[wasm_bindgen(method, getter)]
        pub fn signature(this: &PhantomRequestResponse) -> Option<String>;
    }

    // PhantomError
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen]
        #[derive(Clone, Debug)]
        pub type PhantomError;

        #[wasm_bindgen(method, getter)]
        pub fn message(this: &PhantomError) -> Option<String>;
    }

    // Pubkey
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen]
        #[derive(Clone, Debug)]
        pub type Pubkey;

        #[wasm_bindgen(method, js_name = toBytes)]
        pub fn to_bytes(this: &Pubkey) -> Vec<u8>;

    }

    // Phantom
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(thread_local, js_namespace = window, js_name = solana)]
        pub static SOLANA: Solana;

        #[wasm_bindgen]
        #[derive(Clone)]
        pub type Solana;

        #[wasm_bindgen(method, catch)]
        pub async fn connect(
            this: &Solana,
            options: &JsValue,
        ) -> std::result::Result<JsValue, PhantomError>;

        #[wasm_bindgen(method, getter, js_name = publicKey)]
        pub fn public_key(this: &Solana) -> Pubkey;

        #[wasm_bindgen(method, getter, js_name = isPhantom)]
        pub fn is_phantom(this: &Solana) -> bool;

        #[wasm_bindgen(method, getter, js_name = isConnected)]
        pub fn is_connected(this: &Solana) -> bool;

        #[wasm_bindgen(method, catch)]
        pub fn disconnect(this: &Solana) -> std::result::Result<(), PhantomError>;

        #[wasm_bindgen(method, catch)]
        pub async fn request(
            this: &Solana,
            options: &JsValue,
        ) -> std::result::Result<PhantomRequestResponse, PhantomError>;

        #[wasm_bindgen(method)]
        pub fn on(this: &Solana, event: &str, cb: &js_sys::Function);
        #[wasm_bindgen(method)]
        pub fn off(this: &Solana, event: &str, cb: &js_sys::Function);

    }

    pub fn solana() -> Solana {
        SOLANA.with(|solana| solana.clone())
    }
}

fn console_log(msg: &str) {
    web_sys::console::log_1(&msg.into());
}

// TODO: move this to a wasm shared crate
fn is_ios_redirectable() -> Result<bool> {
    // found at bottom of `wallet-apapter/packages/core/base/adapter`
    Ok(false)
}

// TODO: improve this function and put it into shared crate
async fn sleep_ms(millis: i32) {
    let mut cb = |resolve: js_sys::Function, _reject: js_sys::Function| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, millis)
            .expect("Failed to call set_timeout");
    };
    let p = js_sys::Promise::new(&mut cb);
    wasm_bindgen_futures::JsFuture::from(p).await.unwrap();
}

#[derive(Debug, Clone)]
pub struct PhantomConnectError {
    message: Option<String>,
    error: String,
}

impl From<anyhow::Error> for PhantomConnectError {
    fn from(e: anyhow::Error) -> Self {
        Self {
            message: None,
            error: e.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhantomWallet;

impl PhantomWallet {
    pub fn is_phantom(&self) -> Result<bool> {
        Ok(solana().is_phantom())
    }

    pub fn is_connected(&self) -> Result<bool> {
        Ok(solana().is_connected())
    }

    pub fn disconnect(&self) -> std::result::Result<(), (String, String)> {
        solana().disconnect().map_err(|err| {
            let msg = match err.message() {
                Some(msg) => msg,
                None => "Unknown error".to_string(),
            };

            (msg, format!("{:?}", err))
        })?;
        Ok(())
    }

    pub fn on(&self, event: &str, cb: js_sys::Function) -> Result<()> {
        solana().on(event, &cb);
        Ok(())
    }

    pub fn off(&self, event: &str, cb: js_sys::Function) -> Result<()> {
        solana().off(event, &cb);
        Ok(())
    }

    pub fn public_key(&self) -> Result<Pubkey> {
        console_log("public_key");

        let public_key = solana().public_key();

        let bytes = public_key.to_bytes();

        Ok(bytes.try_into().map_err(|e| anyhow!("{e:?}"))?)
    }

    pub async fn connect(&self) -> std::result::Result<(), PhantomConnectError> {
        console_log("phantom wallet connect");

        let result = solana().connect(&JsValue::NULL).await.map_err(|err| {
            let msg = match err.message() {
                Some(msg) => msg,
                None => "Unknown error".to_string(),
            };

            PhantomConnectError {
                message: Some(msg),
                error: format!("{:?}", err),
            }
        })?;

        tracing::debug!("{:?}", result);

        Ok(())
    }

    // docs found here: https://docs.phantom.app/solana/sending-a-transaction
    pub async fn sign_and_send_transaction(
        &self,
        transaction: TransactionOrVersionedTransaction,
    ) -> Result<solana_sdk::signature::Signature> {
        let tx_bytes = transaction.serialize()?;
        let tx_bs58 = bs58::encode(tx_bytes).into_string();

        console_log(&format!("tx_bs58: {}", tx_bs58));

        let req = PhantomRequest {
            method: "signAndSendTransaction".to_string(),
            params: PhantomRequestParams { message: tx_bs58 },
        };

        let js_value = serde_wasm_bindgen::to_value(&req).map_err(|e| anyhow!("{:?}", e))?;

        console_log(&format!("js_value: {:?}", js_value));

        let resp = solana()
            .request(&js_value)
            .await
            .map_err(|err| anyhow!("{:?}", err))?;

        let signature = resp.signature().context("signature not found")?;

        console_log(&format!("result: {}", signature));

        Ok(signature.parse()?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhantomRequest {
    pub method: String,
    pub params: PhantomRequestParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhantomRequestParams {
    pub message: String,
}

fn set_phantom_url(window: Window) -> std::result::Result<(), JsValue> {
    // redirect to the Phantom /browse universal link
    // this will open the current URL in the Phantom in-wallet browser
    let url = window.location().href()?;
    let origin = window.location().origin()?;

    let href = format!("https://phantom.app/ul/browse/${url}?ref=${origin}");
    window.location().set_href(&href)?;

    Ok(())
}

#[derive(Debug, Clone)]
pub struct PhantomWalletAdapter {
    connecting: Arc<Mutex<bool>>,
    wallet: Arc<Mutex<Option<PhantomWallet>>>,
    public_key: Arc<Mutex<Option<Pubkey>>>,
    wallet_ready_state: Arc<Mutex<WalletReadyState>>,
    account_changed_closure: Arc<Mutex<Option<Closure<dyn FnMut(wallet_binding::Pubkey)>>>>,
    disconnected_closure: Arc<Mutex<Option<Closure<dyn FnMut()>>>>,
    event_emitter: WalletAdapterEventEmitter,
}

impl PhantomWalletAdapter {
    pub fn new() -> Result<Self> {
        let adapter = Self {
            event_emitter: WalletAdapterEventEmitter::new(),
            connecting: Arc::new(Mutex::new(false)),
            wallet: Arc::new(Mutex::new(None)),
            public_key: Arc::new(Mutex::new(None)),
            wallet_ready_state: Arc::new(Mutex::new(WalletReadyState::NotDetected)),
            account_changed_closure: Arc::new(Mutex::new(None)),
            disconnected_closure: Arc::new(Mutex::new(None)),
        };

        if adapter.ready_state() != WalletReadyState::Unsupported {
            if is_ios_redirectable()? {
                *adapter.wallet_ready_state.lock().unwrap() = WalletReadyState::Loadable;
                // js lib emits event here
            } else {
                let self_clone = adapter.clone();

                // TODO: make this waiting loop a shared logic
                wasm_bindgen_futures::spawn_local(async move {
                    for _i in 0..60 {
                        if solana().is_phantom() {
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
                        sleep_ms(1000).await;
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
                console_log("disconnected");
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
                console_log(&format!("account changed: {pubkey:?}"));

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

    fn set_wallet(&self, wallet: Option<PhantomWallet>) {
        *self.wallet.lock().unwrap() = wallet;
    }

    fn set_public_key(&self, public_key: Option<Pubkey>) {
        *self.public_key.lock().unwrap() = public_key;
    }

    fn set_ready_state(&self, ready_state: WalletReadyState) {
        *self.wallet_ready_state.lock().unwrap() = ready_state;
    }

    async fn try_connect(&mut self) -> wallet_adapter_base::Result<()> {
        console_log("phantom connect");

        if self.connected() || self.connecting() {
            return Ok(());
        }

        if self.ready_state() == WalletReadyState::Loadable {
            let window = web_sys::window().context("could not get window")?;
            set_phantom_url(window).map_err(|e| anyhow!("{:?}", e))?;
        }

        if self.ready_state() != WalletReadyState::Installed {
            return Err(wallet_adapter_base::WalletError::WalletNotReady);
        }

        self.set_connecting(true);

        let wallet = PhantomWallet;

        if !wallet.is_connected()? {
            match wallet.connect().await {
                Ok(_) => {}
                Err(e) => {
                    return Err(wallet_adapter_base::WalletError::WalletConnection((
                        e.message.unwrap_or_else(|| "Unknown error".to_string()),
                        e.error,
                    )));
                }
            }
        }

        let public_key = wallet.public_key()?;

        wallet.on("disconnect", self.disconnected())?;
        wallet.on("accountChanged", self.account_changed())?;

        self.set_wallet(Some(wallet));
        self.set_public_key(Some(public_key));

        self.event_emitter
            .emit(WalletAdapterEvent::Connect(public_key))
            .await?;

        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl BaseWalletAdapter for PhantomWalletAdapter {
    fn event_emitter(&self) -> WalletAdapterEventEmitter {
        self.event_emitter.clone()
    }

    fn name(&self) -> String {
        "Phantom".into()
    }

    fn url(&self) -> String {
        "https://phantom.app".into()
    }

    fn icon(&self) -> String {
        "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxMDgiIGhlaWdodD0iMTA4IiB2aWV3Qm94PSIwIDAgMTA4IDEwOCIgZmlsbD0ibm9uZSI+CjxyZWN0IHdpZHRoPSIxMDgiIGhlaWdodD0iMTA4IiByeD0iMjYiIGZpbGw9IiNBQjlGRjIiLz4KPHBhdGggZmlsbC1ydWxlPSJldmVub2RkIiBjbGlwLXJ1bGU9ImV2ZW5vZGQiIGQ9Ik00Ni41MjY3IDY5LjkyMjlDNDIuMDA1NCA3Ni44NTA5IDM0LjQyOTIgODUuNjE4MiAyNC4zNDggODUuNjE4MkMxOS41ODI0IDg1LjYxODIgMTUgODMuNjU2MyAxNSA3NS4xMzQyQzE1IDUzLjQzMDUgNDQuNjMyNiAxOS44MzI3IDcyLjEyNjggMTkuODMyN0M4Ny43NjggMTkuODMyNyA5NCAzMC42ODQ2IDk0IDQzLjAwNzlDOTQgNTguODI1OCA4My43MzU1IDc2LjkxMjIgNzMuNTMyMSA3Ni45MTIyQzcwLjI5MzkgNzYuOTEyMiA2OC43MDUzIDc1LjEzNDIgNjguNzA1MyA3Mi4zMTRDNjguNzA1MyA3MS41NzgzIDY4LjgyNzUgNzAuNzgxMiA2OS4wNzE5IDY5LjkyMjlDNjUuNTg5MyA3NS44Njk5IDU4Ljg2ODUgODEuMzg3OCA1Mi41NzU0IDgxLjM4NzhDNDcuOTkzIDgxLjM4NzggNDUuNjcxMyA3OC41MDYzIDQ1LjY3MTMgNzQuNDU5OEM0NS42NzEzIDcyLjk4ODQgNDUuOTc2OCA3MS40NTU2IDQ2LjUyNjcgNjkuOTIyOVpNODMuNjc2MSA0Mi41Nzk0QzgzLjY3NjEgNDYuMTcwNCA4MS41NTc1IDQ3Ljk2NTggNzkuMTg3NSA0Ny45NjU4Qzc2Ljc4MTYgNDcuOTY1OCA3NC42OTg5IDQ2LjE3MDQgNzQuNjk4OSA0Mi41Nzk0Qzc0LjY5ODkgMzguOTg4NSA3Ni43ODE2IDM3LjE5MzEgNzkuMTg3NSAzNy4xOTMxQzgxLjU1NzUgMzcuMTkzMSA4My42NzYxIDM4Ljk4ODUgODMuNjc2MSA0Mi41Nzk0Wk03MC4yMTAzIDQyLjU3OTVDNzAuMjEwMyA0Ni4xNzA0IDY4LjA5MTYgNDcuOTY1OCA2NS43MjE2IDQ3Ljk2NThDNjMuMzE1NyA0Ny45NjU4IDYxLjIzMyA0Ni4xNzA0IDYxLjIzMyA0Mi41Nzk1QzYxLjIzMyAzOC45ODg1IDYzLjMxNTcgMzcuMTkzMSA2NS43MjE2IDM3LjE5MzFDNjguMDkxNiAzNy4xOTMxIDcwLjIxMDMgMzguOTg4NSA3MC4yMTAzIDQyLjU3OTVaIiBmaWxsPSIjRkZGREY4Ii8+Cjwvc3ZnPg==".into()
    }

    fn connected(&self) -> bool {
        self.wallet
            .lock()
            .unwrap()
            .as_ref()
            .map_or(false, |w| w.is_connected().unwrap())
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
        let opt_wallet = { self.wallet.lock().unwrap().as_ref().cloned() };
        let Some(wallet) = opt_wallet else {
            return Ok(());
        };

        wallet.off("disconnect", self.disconnected())?;
        wallet.off("accountChanged", self.account_changed())?;

        self.set_wallet(None);
        self.set_public_key(None);

        if let Err((err_msg, err)) = wallet.disconnect() {
            self.event_emitter
                .emit(WalletAdapterEvent::Error(WalletError::WalletDisconnection(
                    (err_msg, err),
                )))
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
        let opt_wallet = { self.wallet.lock().unwrap().as_ref().cloned() };

        let Some(wallet) = opt_wallet else {
            return Err(wallet_adapter_base::WalletError::WalletNotConnected);
        };

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

        Ok(wallet.sign_and_send_transaction(transaction).await?)
    }
}
