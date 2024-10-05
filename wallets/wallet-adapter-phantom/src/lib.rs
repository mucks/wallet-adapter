use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use js_sys::{Atomics::wait_async_with_timeout_bigint, Object};
use solana_sdk::{pubkey::Pubkey, transaction::TransactionVersion};
use wallet_adapter_base::{BaseWalletAdapter, SupportedTransactionVersions, WalletReadyState};
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};
use web_sys::{console, Window};

fn console_log(msg: &str) {
    web_sys::console::log_1(&msg.into());
}

// TODO: move this to a wasm shared crate
fn is_ios_redirectable() -> Result<bool> {
    // found at bottom of `wallet-apapter/packages/core/base/adapter`
    Ok(false)
}

// TODO: move this to a wasm shared crate
fn reflect_get(target: &JsValue, key: &JsValue) -> Result<JsValue> {
    let result = js_sys::Reflect::get(target, key).map_err(|e| anyhow!("{:?}", e))?;
    log::debug!("reflect_get: {:?}", result);
    Ok(result)
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
pub struct PhantomWallet(Object);

impl PhantomWallet {
    pub fn from_window(window: Window) -> Result<Self> {
        let solana = window
            .get("solana")
            .context("could not get solana object")?;
        Ok(Self(solana))
    }

    pub fn is_phantom(&self) -> Result<bool> {
        reflect_get(&self.0, &JsValue::from_str("isPhantom"))?
            .as_bool()
            .context("isConnected is not a bool")
    }

    pub fn is_connected(&self) -> Result<bool> {
        reflect_get(&self.0, &JsValue::from_str("isConnected"))?
            .as_bool()
            .context("isConnected is not a bool")
    }

    pub fn on(&self, event: &str, cb: js_sys::Function) -> Result<()> {
        let on: js_sys::Function = reflect_get(&self.0, &JsValue::from_str("on"))?.into();
        on.call2(&self.0, &JsValue::from_str(event), &cb)
            .map_err(|err| anyhow!("{:?}", err))?;
        Ok(())
    }

    pub fn public_key(&self) -> Result<Pubkey> {
        console_log("public_key");

        let public_key = reflect_get(&self.0, &JsValue::from_str("publicKey"))?;
        let to_bytes: js_sys::Function =
            reflect_get(&public_key, &JsValue::from_str("toBytes"))?.into();

        let bytes = to_bytes
            .call0(&public_key)
            .map_err(|err| anyhow!("{:?}", err))?;

        let bytes = js_sys::Uint8Array::new(&bytes).to_vec();

        Ok(bytes.try_into().map_err(|e| anyhow!("{e:?}"))?)
    }

    pub async fn connect(&self) -> std::result::Result<(), PhantomConnectError> {
        console_log("phantom wallet connect");

        let connect_str = wasm_bindgen::JsValue::from_str("connect");
        let connect: js_sys::Function = reflect_get(&self.0, &connect_str)?.into();

        log::debug!("{:?}", connect.to_string());

        let resp = connect.call0(&self.0).map_err(|err| {
            if let Some(message) = reflect_get(&err, &JsValue::from_str("message"))
                .ok()
                .and_then(|msg| msg.as_string())
            {
                PhantomConnectError {
                    message: Some(message),
                    error: "connect error".to_string(),
                }
            } else {
                PhantomConnectError {
                    message: None,
                    error: "connect error".to_string(),
                }
            }
        })?;

        let promise = js_sys::Promise::resolve(&resp);

        let result = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(|err| anyhow!("{err:?}"))?;

        log::debug!("{:?}", result);

        Ok(())
    }
}

fn window() -> Result<Window> {
    web_sys::window().context("could not get window")
}

fn is_phantom() -> Result<bool> {
    if let Some(solana) = get_wallet_object()? {
        let is_phantom = reflect_get(&solana, &JsValue::from_str("isPhantom"))?;
        return Ok(is_phantom.is_truthy());
    }

    Ok(false)
}

fn get_wallet_object() -> Result<Option<js_sys::Object>> {
    Ok(window()?.get("solana"))
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
}

impl PhantomWalletAdapter {
    pub fn new() -> Result<Self> {
        let adapter = Self {
            connecting: Arc::new(Mutex::new(false)),
            wallet: Arc::new(Mutex::new(None)),
            public_key: Arc::new(Mutex::new(None)),
            wallet_ready_state: Arc::new(Mutex::new(WalletReadyState::NotDetected)),
        };

        if adapter.ready_state() != WalletReadyState::Unsupported {
            if is_ios_redirectable()? {
                *adapter.wallet_ready_state.lock().unwrap() = WalletReadyState::Loadable;
                // js lib emits event here
            } else {
                let ready_state = adapter.wallet_ready_state.clone();

                // TODO: make this waiting loop a shared logic
                wasm_bindgen_futures::spawn_local(async move {
                    for _i in 0..60 {
                        if let Ok(true) = is_phantom() {
                            *ready_state.lock().unwrap() = WalletReadyState::Installed;
                            break;
                        }
                        sleep_ms(1000).await;
                    }
                });
            }
        }

        Ok(adapter)
    }

    fn account_changed(pubkey: Pubkey) {
        // js lib emits event here
    }

    fn set_connecting(&self, connecting: bool) {
        *self.connecting.lock().unwrap() = connecting;
    }

    fn set_wallet(&self, wallet: PhantomWallet) {
        *self.wallet.lock().unwrap() = Some(wallet);
    }

    fn set_public_key(&self, public_key: Pubkey) {
        *self.public_key.lock().unwrap() = Some(public_key);
    }
}

impl BaseWalletAdapter for PhantomWalletAdapter {
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
        console_log("phantom connect");

        if self.connected() || self.connecting() {
            return Ok(());
        }

        if self.ready_state() == WalletReadyState::Loadable {
            let window = web_sys::window().context("could not get window")?;
            set_phantom_url(window).map_err(|e| anyhow!("{:?}", e))?;
        }

        if self.ready_state() != WalletReadyState::Installed {
            return Err(wallet_adapter_base::Error::WalletNotReady);
        }

        self.set_connecting(true);

        let wallet = PhantomWallet::from_window(window()?)?;

        if !wallet.is_connected()? {
            match wallet.connect().await {
                Ok(_) => {}
                Err(e) => {
                    return Err(wallet_adapter_base::Error::WalletConnection((
                        e.message.unwrap_or_else(|| "Unknown error".to_string()),
                        e.error,
                    )));
                }
            }
        }

        let public_key = wallet.public_key()?;

        let connected = Closure::wrap(Box::new(move || {
            println!("wallet connected");
        }) as Box<dyn FnMut()>);

        let connected_: &js_sys::Function = connected.as_ref().unchecked_ref();

        wallet.on("disconnect", connected_.clone())?;

        wallet.on("connect", connected_.clone())?;

        self.set_wallet(wallet);
        self.set_public_key(public_key);
        self.set_connecting(false);

        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        todo!()
    }

    async fn send_transaction(
        &self,
        transaction: wallet_adapter_base::TransactionOrVersionedTransaction,
        connection: impl wallet_adapter_web3::Connection,
        options: Option<wallet_adapter_web3::SendTransactionOptions>,
    ) -> Result<solana_sdk::signature::Signature> {
        todo!()
    }
}
