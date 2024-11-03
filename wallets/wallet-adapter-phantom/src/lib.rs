use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::{bs58, pubkey::Pubkey};
use wallet_adapter_base::{BaseWalletAdapter, TransactionOrVersionedTransaction};
use wallet_adapter_wasm::generic_wallet::{GenericWasmWallet, GenericWasmWalletAdapter};
use wallet_adapter_wasm::util::reflect_get;
use wallet_binding::solana;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
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

#[derive(Debug, Clone, PartialEq)]
pub struct PhantomWallet;

#[async_trait::async_trait(?Send)]
impl GenericWasmWallet for PhantomWallet {
    fn name(&self) -> String {
        "Phantom".to_string()
    }

    fn url(&self) -> String {
        "https://phantom.app".into()
    }

    fn icon(&self) -> String {
        "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxMDgiIGhlaWdodD0iMTA4IiB2aWV3Qm94PSIwIDAgMTA4IDEwOCIgZmlsbD0ibm9uZSI+CjxyZWN0IHdpZHRoPSIxMDgiIGhlaWdodD0iMTA4IiByeD0iMjYiIGZpbGw9IiNBQjlGRjIiLz4KPHBhdGggZmlsbC1ydWxlPSJldmVub2RkIiBjbGlwLXJ1bGU9ImV2ZW5vZGQiIGQ9Ik00Ni41MjY3IDY5LjkyMjlDNDIuMDA1NCA3Ni44NTA5IDM0LjQyOTIgODUuNjE4MiAyNC4zNDggODUuNjE4MkMxOS41ODI0IDg1LjYxODIgMTUgODMuNjU2MyAxNSA3NS4xMzQyQzE1IDUzLjQzMDUgNDQuNjMyNiAxOS44MzI3IDcyLjEyNjggMTkuODMyN0M4Ny43NjggMTkuODMyNyA5NCAzMC42ODQ2IDk0IDQzLjAwNzlDOTQgNTguODI1OCA4My43MzU1IDc2LjkxMjIgNzMuNTMyMSA3Ni45MTIyQzcwLjI5MzkgNzYuOTEyMiA2OC43MDUzIDc1LjEzNDIgNjguNzA1MyA3Mi4zMTRDNjguNzA1MyA3MS41NzgzIDY4LjgyNzUgNzAuNzgxMiA2OS4wNzE5IDY5LjkyMjlDNjUuNTg5MyA3NS44Njk5IDU4Ljg2ODUgODEuMzg3OCA1Mi41NzU0IDgxLjM4NzhDNDcuOTkzIDgxLjM4NzggNDUuNjcxMyA3OC41MDYzIDQ1LjY3MTMgNzQuNDU5OEM0NS42NzEzIDcyLjk4ODQgNDUuOTc2OCA3MS40NTU2IDQ2LjUyNjcgNjkuOTIyOVpNODMuNjc2MSA0Mi41Nzk0QzgzLjY3NjEgNDYuMTcwNCA4MS41NTc1IDQ3Ljk2NTggNzkuMTg3NSA0Ny45NjU4Qzc2Ljc4MTYgNDcuOTY1OCA3NC42OTg5IDQ2LjE3MDQgNzQuNjk4OSA0Mi41Nzk0Qzc0LjY5ODkgMzguOTg4NSA3Ni43ODE2IDM3LjE5MzEgNzkuMTg3NSAzNy4xOTMxQzgxLjU1NzUgMzcuMTkzMSA4My42NzYxIDM4Ljk4ODUgODMuNjc2MSA0Mi41Nzk0Wk03MC4yMTAzIDQyLjU3OTVDNzAuMjEwMyA0Ni4xNzA0IDY4LjA5MTYgNDcuOTY1OCA2NS43MjE2IDQ3Ljk2NThDNjMuMzE1NyA0Ny45NjU4IDYxLjIzMyA0Ni4xNzA0IDYxLjIzMyA0Mi41Nzk1QzYxLjIzMyAzOC45ODg1IDYzLjMxNTcgMzcuMTkzMSA2NS43MjE2IDM3LjE5MzFDNjguMDkxNiAzNy4xOTMxIDcwLjIxMDMgMzguOTg4NSA3MC4yMTAzIDQyLjU3OTVaIiBmaWxsPSIjRkZGREY4Ii8+Cjwvc3ZnPg==".into()
    }

    fn is_correct_wallet(&self) -> bool {
        match reflect_get(&solana(), &JsValue::from_str("isPhantom")) {
            Ok(val) => val.as_bool().unwrap_or(false),
            Err(_) => false,
        }
    }

    fn is_connected(&self) -> bool {
        solana().is_connected()
    }

    fn disconnect(&self) -> Result<()> {
        solana()
            .disconnect()
            .map_err(|err| anyhow!("{:?}", err).into())
    }

    fn on(&self, event: &str, cb: js_sys::Function) -> Result<()> {
        solana().on(event, &cb);
        Ok(())
    }

    fn off(&self, event: &str, cb: js_sys::Function) -> Result<()> {
        solana().off(event, &cb);
        Ok(())
    }

    fn public_key(&self) -> Result<Pubkey> {
        tracing::debug!("public_key");

        let public_key = solana().public_key();

        let bytes = public_key.to_bytes();

        Ok(bytes.try_into().map_err(|e| anyhow!("{e:?}"))?)
    }

    async fn connect(&self) -> Result<()> {
        tracing::debug!("phantom wallet connect");

        let result = solana()
            .connect(&JsValue::NULL)
            .await
            .map_err(|err| anyhow!("{:?}", err))?;

        tracing::debug!("{:?}", result);

        Ok(())
    }

    // docs found here: https://docs.phantom.app/solana/sending-a-transaction
    async fn sign_and_send_transaction(
        &self,
        transaction: TransactionOrVersionedTransaction,
    ) -> Result<solana_sdk::signature::Signature> {
        let tx_bytes = transaction.serialize()?;
        let tx_bs58 = bs58::encode(tx_bytes).into_string();

        tracing::debug!("tx_bs58: {}", tx_bs58);

        let req = PhantomRequest {
            method: "signAndSendTransaction".to_string(),
            params: PhantomRequestParams { message: tx_bs58 },
        };

        let js_value = serde_wasm_bindgen::to_value(&req).map_err(|e| anyhow!("{:?}", e))?;

        tracing::debug!("js_value: {:?}", js_value);

        let resp = solana()
            .request(&js_value)
            .await
            .map_err(|err| anyhow!("{:?}", err))?;

        let signature = resp.signature().context("signature not found")?;

        tracing::debug!("result: {}", signature);

        Ok(signature.parse()?)
    }

    fn is_ios_redirectable(&self) -> Result<bool> {
        Ok(false)
    }
    fn set_wallet_url(&self) -> Result<()> {
        set_phantom_url(web_sys::window().context("could not get window")?)
            .map_err(|e| anyhow!("{:?}", e))
    }
}

pub struct PhantomWalletAdapter {
    adapter: GenericWasmWalletAdapter<PhantomWallet>,
}

impl PhantomWalletAdapter {
    pub fn new() -> Result<Self> {
        Ok(Self {
            adapter: GenericWasmWalletAdapter::new(PhantomWallet)?,
        })
    }

    pub fn to_dyn_adapter(&self) -> Box<dyn BaseWalletAdapter> {
        Box::new(self.adapter.clone())
    }
}
