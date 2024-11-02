// TODO: a lot of code here is shared with phantom wallet adapter, move to shared crate!

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

mod wallet_binding {
    use super::*;

    // SolflareRequestResponse
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen]
        #[derive(Clone, Debug)]
        pub type SolflareRequestResponse;

        #[wasm_bindgen(method, getter)]
        pub fn signature(this: &SolflareRequestResponse) -> Option<String>;
    }

    // SolflareError
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen]
        #[derive(Clone, Debug)]
        pub type SolflareError;

        #[wasm_bindgen(method, getter)]
        pub fn message(this: &SolflareError) -> Option<String>;
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

    // Solflare
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(thread_local, js_namespace = window, js_name = solflare)]
        pub static SOLFLARE: Solana;

        #[wasm_bindgen]
        #[derive(Clone)]
        pub type Solana;

        #[wasm_bindgen(method, catch)]
        pub async fn connect(
            this: &Solana,
            options: &JsValue,
        ) -> std::result::Result<JsValue, SolflareError>;

        #[wasm_bindgen(method, getter, js_name = publicKey)]
        pub fn public_key(this: &Solana) -> Pubkey;

        #[wasm_bindgen(method, getter, js_name = isSolflare)]
        pub fn is_solflare(this: &Solana) -> bool;

        #[wasm_bindgen(method, getter, js_name = isConnected)]
        pub fn is_connected(this: &Solana) -> bool;

        #[wasm_bindgen(method, catch)]
        pub fn disconnect(this: &Solana) -> std::result::Result<(), SolflareError>;

        #[wasm_bindgen(method, catch)]
        pub async fn request(
            this: &Solana,
            options: &JsValue,
        ) -> std::result::Result<SolflareRequestResponse, SolflareError>;

        #[wasm_bindgen(method)]
        pub fn on(this: &Solana, event: &str, cb: &js_sys::Function);
        #[wasm_bindgen(method)]
        pub fn off(this: &Solana, event: &str, cb: &js_sys::Function);

    }

    pub fn solana() -> Solana {
        SOLFLARE.with(|solana| solana.clone())
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
pub struct SolflareConnectError {
    message: Option<String>,
    error: String,
}

impl From<anyhow::Error> for SolflareConnectError {
    fn from(e: anyhow::Error) -> Self {
        Self {
            message: None,
            error: e.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SolflareWallet;

impl SolflareWallet {
    pub fn _is_solflare(&self) -> Result<bool> {
        Ok(solana().is_solflare())
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

    pub async fn connect(&self) -> std::result::Result<(), SolflareConnectError> {
        console_log("solflare wallet connect");

        let result = solana().connect(&JsValue::NULL).await.map_err(|err| {
            let msg = match err.message() {
                Some(msg) => msg,
                None => "Unknown error".to_string(),
            };

            SolflareConnectError {
                message: Some(msg),
                error: format!("{:?}", err),
            }
        })?;

        tracing::debug!("{:?}", result);

        Ok(())
    }

    pub async fn sign_and_send_transaction(
        &self,
        transaction: TransactionOrVersionedTransaction,
    ) -> Result<solana_sdk::signature::Signature> {
        let tx_bytes = transaction.serialize()?;
        let tx_bs58 = bs58::encode(tx_bytes).into_string();

        console_log(&format!("tx_bs58: {}", tx_bs58));

        let req = SolflareRequest {
            method: "signAndSendTransaction".to_string(),
            params: SolflareRequestParams {
                transaction: tx_bs58,
            },
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
pub struct SolflareRequest {
    pub method: String,
    pub params: SolflareRequestParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolflareRequestParams {
    pub transaction: String,
}

#[derive(Debug, Clone)]
pub struct SolflareWalletAdapter {
    connecting: Arc<Mutex<bool>>,
    wallet: Arc<Mutex<Option<SolflareWallet>>>,
    public_key: Arc<Mutex<Option<Pubkey>>>,
    wallet_ready_state: Arc<Mutex<WalletReadyState>>,
    account_changed_closure: Arc<Mutex<Option<Closure<dyn FnMut(wallet_binding::Pubkey)>>>>,
    disconnected_closure: Arc<Mutex<Option<Closure<dyn FnMut()>>>>,
    event_emitter: WalletAdapterEventEmitter,
}

impl SolflareWalletAdapter {
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
                        if solana().is_solflare() {
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

    fn set_wallet(&self, wallet: Option<SolflareWallet>) {
        *self.wallet.lock().unwrap() = wallet;
    }

    fn set_public_key(&self, public_key: Option<Pubkey>) {
        *self.public_key.lock().unwrap() = public_key;
    }

    fn set_ready_state(&self, ready_state: WalletReadyState) {
        *self.wallet_ready_state.lock().unwrap() = ready_state;
    }

    async fn try_connect(&mut self) -> wallet_adapter_base::Result<()> {
        console_log("solflare connect");

        if self.connected() || self.connecting() {
            return Ok(());
        }

        if self.ready_state() != WalletReadyState::Installed {
            return Err(wallet_adapter_base::WalletError::WalletNotReady);
        }

        self.set_connecting(true);

        let wallet = SolflareWallet;

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
impl BaseWalletAdapter for SolflareWalletAdapter {
    fn event_emitter(&self) -> WalletAdapterEventEmitter {
        self.event_emitter.clone()
    }

    fn name(&self) -> String {
        "Solflare".into()
    }

    fn url(&self) -> String {
        "https://solflare.com".into()
    }

    fn icon(&self) -> String {
        "data:image/svg+xml;base64,PHN2ZyBmaWxsPSJub25lIiBoZWlnaHQ9IjUwIiB2aWV3Qm94PSIwIDAgNTAgNTAiIHdpZHRoPSI1MCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIiB4bWxuczp4bGluaz0iaHR0cDovL3d3dy53My5vcmcvMTk5OS94bGluayI+PGxpbmVhckdyYWRpZW50IGlkPSJhIj48c3RvcCBvZmZzZXQ9IjAiIHN0b3AtY29sb3I9IiNmZmMxMGIiLz48c3RvcCBvZmZzZXQ9IjEiIHN0b3AtY29sb3I9IiNmYjNmMmUiLz48L2xpbmVhckdyYWRpZW50PjxsaW5lYXJHcmFkaWVudCBpZD0iYiIgZ3JhZGllbnRVbml0cz0idXNlclNwYWNlT25Vc2UiIHgxPSI2LjQ3ODM1IiB4Mj0iMzQuOTEwNyIgeGxpbms6aHJlZj0iI2EiIHkxPSI3LjkyIiB5Mj0iMzMuNjU5MyIvPjxyYWRpYWxHcmFkaWVudCBpZD0iYyIgY3g9IjAiIGN5PSIwIiBncmFkaWVudFRyYW5zZm9ybT0ibWF0cml4KDQuOTkyMTg4MzIgMTIuMDYzODc5NjMgLTEyLjE4MTEzNjU1IDUuMDQwNzEwNzQgMjIuNTIwMiAyMC42MTgzKSIgZ3JhZGllbnRVbml0cz0idXNlclNwYWNlT25Vc2UiIHI9IjEiIHhsaW5rOmhyZWY9IiNhIi8+PHBhdGggZD0ibTI1LjE3MDggNDcuOTEwNGMuNTI1IDAgLjk1MDcuNDIxLjk1MDcuOTQwM3MtLjQyNTcuOTQwMi0uOTUwNy45NDAyLS45NTA3LS40MjA5LS45NTA3LS45NDAyLjQyNTctLjk0MDMuOTUwNy0uOTQwM3ptLTEuMDMyOC00NC45MTU2NWMuNDY0Ni4wMzgzNi44Mzk4LjM5MDQuOTAyNy44NDY4MWwxLjEzMDcgOC4yMTU3NGMuMzc5OCAyLjcxNDMgMy42NTM1IDMuODkwNCA1LjY3NDMgMi4wNDU5bDExLjMyOTEtMTAuMzExNThjLjI3MzMtLjI0ODczLjY5ODktLjIzMTQ5Ljk1MDcuMDM4NTEuMjMwOS4yNDc3Mi4yMzc5LjYyNjk3LjAxNjEuODgyNzdsLTkuODc5MSAxMS4zOTU4Yy0xLjgxODcgMi4wOTQyLS40NzY4IDUuMzY0MyAyLjI5NTYgNS41OTc4bDguNzE2OC44NDAzYy40MzQxLjA0MTguNzUxNy40MjM0LjcwOTMuODUyNC0uMDM0OS4zNTM3LS4zMDc0LjYzOTUtLjY2MjguNjk0OWwtOS4xNTk0IDEuNDMwMmMtMi42NTkzLjM2MjUtMy44NjM2IDMuNTExNy0yLjEzMzkgNS41NTc2bDMuMjIgMy43OTYxYy4yNTk0LjMwNTguMjE4OC43NjE1LS4wOTA4IDEuMDE3OC0uMjYyMi4yMTcyLS42NDE5LjIyNTYtLjkxMzguMDIwM2wtMy45Njk0LTIuOTk3OGMtMi4xNDIxLTEuNjEwOS01LjIyOTctLjI0MTctNS40NTYxIDIuNDI0M2wtLjg3NDcgMTAuMzk3NmMtLjAzNjIuNDI5NS0uNDE3OC43NDg3LS44NTI1LjcxMy0uMzY5LS4wMzAzLS42NjcxLS4zMDk3LS43MTcxLS42NzIxbC0xLjM4NzEtMTAuMDQzN2MtLjM3MTctMi43MTQ0LTMuNjQ1NC0zLjg5MDQtNS42NzQzLTIuMDQ1OWwtMTIuMDUxOTUgMTAuOTc0Yy0uMjQ5NDcuMjI3MS0uNjM4MDkuMjExNC0uODY4LS4wMzUtLjIxMDk0LS4yMjYyLS4yMTczNS0uNTcyNC0uMDE0OTMtLjgwNmwxMC41MTgxOC0xMi4xMzg1YzEuODE4Ny0yLjA5NDIuNDg0OS01LjM2NDQtMi4yODc2LTUuNTk3OGwtOC43MTg3Mi0uODQwNWMtLjQzNDEzLS4wNDE4LS43NTE3Mi0uNDIzNS0uNzA5MzYtLjg1MjQuMDM0OTMtLjM1MzcuMzA3MzktLjYzOTQuNjYyNy0uNjk1bDkuMTUzMzgtMS40Mjk5YzIuNjU5NC0uMzYyNSAzLjg3MTgtMy41MTE3IDIuMTQyMS01LjU1NzZsLTIuMTkyLTIuNTg0MWMtLjMyMTctLjM3OTItLjI3MTMtLjk0NDMuMTEyNi0xLjI2MjEuMzI1My0uMjY5NC43OTYzLS4yNzk3IDEuMTMzNC0uMDI0OWwyLjY5MTggMi4wMzQ3YzIuMTQyMSAxLjYxMDkgNS4yMjk3LjI0MTcgNS40NTYxLTIuNDI0M2wuNzI0MS04LjU1OTk4Yy4wNDU3LS41NDA4LjUyNjUtLjk0MjU3IDEuMDczOS0uODk3Mzd6bS0yMy4xODczMyAyMC40Mzk2NWMuNTI1MDQgMCAuOTUwNjcuNDIxLjk1MDY3Ljk0MDNzLS40MjU2My45NDAzLS45NTA2Ny45NDAzYy0uNTI1MDQxIDAtLjk1MDY3LS40MjEtLjk1MDY3LS45NDAzcy40MjU2MjktLjk0MDMuOTUwNjctLjk0MDN6bTQ3LjY3OTczLS45NTQ3Yy41MjUgMCAuOTUwNy40MjEuOTUwNy45NDAzcy0uNDI1Ny45NDAyLS45NTA3Ljk0MDItLjk1MDctLjQyMDktLjk1MDctLjk0MDIuNDI1Ny0uOTQwMy45NTA3LS45NDAzem0tMjQuNjI5Ni0yMi40Nzk3Yy41MjUgMCAuOTUwNi40MjA5NzMuOTUwNi45NDAyNyAwIC41MTkzLS40MjU2Ljk0MDI3LS45NTA2Ljk0MDI3LS41MjUxIDAtLjk1MDctLjQyMDk3LS45NTA3LS45NDAyNyAwLS41MTkyOTcuNDI1Ni0uOTQwMjcuOTUwNy0uOTQwMjd6IiBmaWxsPSJ1cmwoI2IpIi8+PHBhdGggZD0ibTI0LjU3MSAzMi43NzkyYzQuOTU5NiAwIDguOTgwMi0zLjk3NjUgOC45ODAyLTguODgxOSAwLTQuOTA1My00LjAyMDYtOC44ODE5LTguOTgwMi04Ljg4MTlzLTguOTgwMiAzLjk3NjYtOC45ODAyIDguODgxOWMwIDQuOTA1NCA0LjAyMDYgOC44ODE5IDguOTgwMiA4Ljg4MTl6IiBmaWxsPSJ1cmwoI2MpIi8+PC9zdmc+".into()
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
