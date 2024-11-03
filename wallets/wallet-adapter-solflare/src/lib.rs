use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::{bs58, pubkey::Pubkey};
use wallet_adapter_base::{BaseWalletAdapter, TransactionOrVersionedTransaction};
use wallet_adapter_wasm::generic_wallet::{GenericWasmWallet, GenericWasmWalletAdapter};
use wallet_adapter_wasm::util::reflect_get;
use wallet_binding::solana;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

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
        #[wasm_bindgen(thread_local, js_namespace = window, js_name = solflare)]
        pub static SOLFLARE: Solana;

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
        SOLFLARE.with(|solana| solana.clone())
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

#[derive(Debug, Clone, PartialEq)]
pub struct SolflareWallet;

#[async_trait::async_trait(?Send)]
impl GenericWasmWallet for SolflareWallet {
    fn name(&self) -> String {
        "Solflare".into()
    }

    fn url(&self) -> String {
        "https://solflare.com".into()
    }

    fn icon(&self) -> String {
        "data:image/svg+xml;base64,PHN2ZyBmaWxsPSJub25lIiBoZWlnaHQ9IjUwIiB2aWV3Qm94PSIwIDAgNTAgNTAiIHdpZHRoPSI1MCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIiB4bWxuczp4bGluaz0iaHR0cDovL3d3dy53My5vcmcvMTk5OS94bGluayI+PGxpbmVhckdyYWRpZW50IGlkPSJhIj48c3RvcCBvZmZzZXQ9IjAiIHN0b3AtY29sb3I9IiNmZmMxMGIiLz48c3RvcCBvZmZzZXQ9IjEiIHN0b3AtY29sb3I9IiNmYjNmMmUiLz48L2xpbmVhckdyYWRpZW50PjxsaW5lYXJHcmFkaWVudCBpZD0iYiIgZ3JhZGllbnRVbml0cz0idXNlclNwYWNlT25Vc2UiIHgxPSI2LjQ3ODM1IiB4Mj0iMzQuOTEwNyIgeGxpbms6aHJlZj0iI2EiIHkxPSI3LjkyIiB5Mj0iMzMuNjU5MyIvPjxyYWRpYWxHcmFkaWVudCBpZD0iYyIgY3g9IjAiIGN5PSIwIiBncmFkaWVudFRyYW5zZm9ybT0ibWF0cml4KDQuOTkyMTg4MzIgMTIuMDYzODc5NjMgLTEyLjE4MTEzNjU1IDUuMDQwNzEwNzQgMjIuNTIwMiAyMC42MTgzKSIgZ3JhZGllbnRVbml0cz0idXNlclNwYWNlT25Vc2UiIHI9IjEiIHhsaW5rOmhyZWY9IiNhIi8+PHBhdGggZD0ibTI1LjE3MDggNDcuOTEwNGMuNTI1IDAgLjk1MDcuNDIxLjk1MDcuOTQwM3MtLjQyNTcuOTQwMi0uOTUwNy45NDAyLS45NTA3LS40MjA5LS45NTA3LS45NDAyLjQyNTctLjk0MDMuOTUwNy0uOTQwM3ptLTEuMDMyOC00NC45MTU2NWMuNDY0Ni4wMzgzNi44Mzk4LjM5MDQuOTAyNy44NDY4MWwxLjEzMDcgOC4yMTU3NGMuMzc5OCAyLjcxNDMgMy42NTM1IDMuODkwNCA1LjY3NDMgMi4wNDU5bDExLjMyOTEtMTAuMzExNThjLjI3MzMtLjI0ODczLjY5ODktLjIzMTQ5Ljk1MDcuMDM4NTEuMjMwOS4yNDc3Mi4yMzc5LjYyNjk3LjAxNjEuODgyNzdsLTkuODc5MSAxMS4zOTU4Yy0xLjgxODcgMi4wOTQyLS40NzY4IDUuMzY0MyAyLjI5NTYgNS41OTc4bDguNzE2OC44NDAzYy40MzQxLjA0MTguNzUxNy40MjM0LjcwOTMuODUyNC0uMDM0OS4zNTM3LS4zMDc0LjYzOTUtLjY2MjguNjk0OWwtOS4xNTk0IDEuNDMwMmMtMi42NTkzLjM2MjUtMy44NjM2IDMuNTExNy0yLjEzMzkgNS41NTc2bDMuMjIgMy43OTYxYy4yNTk0LjMwNTguMjE4OC43NjE1LS4wOTA4IDEuMDE3OC0uMjYyMi4yMTcyLS42NDE5LjIyNTYtLjkxMzguMDIwM2wtMy45Njk0LTIuOTk3OGMtMi4xNDIxLTEuNjEwOS01LjIyOTctLjI0MTctNS40NTYxIDIuNDI0M2wtLjg3NDcgMTAuMzk3NmMtLjAzNjIuNDI5NS0uNDE3OC43NDg3LS44NTI1LjcxMy0uMzY5LS4wMzAzLS42NjcxLS4zMDk3LS43MTcxLS42NzIxbC0xLjM4NzEtMTAuMDQzN2MtLjM3MTctMi43MTQ0LTMuNjQ1NC0zLjg5MDQtNS42NzQzLTIuMDQ1OWwtMTIuMDUxOTUgMTAuOTc0Yy0uMjQ5NDcuMjI3MS0uNjM4MDkuMjExNC0uODY4LS4wMzUtLjIxMDk0LS4yMjYyLS4yMTczNS0uNTcyNC0uMDE0OTMtLjgwNmwxMC41MTgxOC0xMi4xMzg1YzEuODE4Ny0yLjA5NDIuNDg0OS01LjM2NDQtMi4yODc2LTUuNTk3OGwtOC43MTg3Mi0uODQwNWMtLjQzNDEzLS4wNDE4LS43NTE3Mi0uNDIzNS0uNzA5MzYtLjg1MjQuMDM0OTMtLjM1MzcuMzA3MzktLjYzOTQuNjYyNy0uNjk1bDkuMTUzMzgtMS40Mjk5YzIuNjU5NC0uMzYyNSAzLjg3MTgtMy41MTE3IDIuMTQyMS01LjU1NzZsLTIuMTkyLTIuNTg0MWMtLjMyMTctLjM3OTItLjI3MTMtLjk0NDMuMTEyNi0xLjI2MjEuMzI1My0uMjY5NC43OTYzLS4yNzk3IDEuMTMzNC0uMDI0OWwyLjY5MTggMi4wMzQ3YzIuMTQyMSAxLjYxMDkgNS4yMjk3LjI0MTcgNS40NTYxLTIuNDI0M2wuNzI0MS04LjU1OTk4Yy4wNDU3LS41NDA4LjUyNjUtLjk0MjU3IDEuMDczOS0uODk3Mzd6bS0yMy4xODczMyAyMC40Mzk2NWMuNTI1MDQgMCAuOTUwNjcuNDIxLjk1MDY3Ljk0MDNzLS40MjU2My45NDAzLS45NTA2Ny45NDAzYy0uNTI1MDQxIDAtLjk1MDY3LS40MjEtLjk1MDY3LS45NDAzcy40MjU2MjktLjk0MDMuOTUwNjctLjk0MDN6bTQ3LjY3OTczLS45NTQ3Yy41MjUgMCAuOTUwNy40MjEuOTUwNy45NDAzcy0uNDI1Ny45NDAyLS45NTA3Ljk0MDItLjk1MDctLjQyMDktLjk1MDctLjk0MDIuNDI1Ny0uOTQwMy45NTA3LS45NDAzem0tMjQuNjI5Ni0yMi40Nzk3Yy41MjUgMCAuOTUwNi40MjA5NzMuOTUwNi45NDAyNyAwIC41MTkzLS40MjU2Ljk0MDI3LS45NTA2Ljk0MDI3LS41MjUxIDAtLjk1MDctLjQyMDk3LS45NTA3LS45NDAyNyAwLS41MTkyOTcuNDI1Ni0uOTQwMjcuOTUwNy0uOTQwMjd6IiBmaWxsPSJ1cmwoI2IpIi8+PHBhdGggZD0ibTI0LjU3MSAzMi43NzkyYzQuOTU5NiAwIDguOTgwMi0zLjk3NjUgOC45ODAyLTguODgxOSAwLTQuOTA1My00LjAyMDYtOC44ODE5LTguOTgwMi04Ljg4MTlzLTguOTgwMiAzLjk3NjYtOC45ODAyIDguODgxOWMwIDQuOTA1NCA0LjAyMDYgOC44ODE5IDguOTgwMiA4Ljg4MTl6IiBmaWxsPSJ1cmwoI2MpIi8+PC9zdmc+".into()
    }

    fn is_correct_wallet(&self) -> bool {
        match reflect_get(&solana(), &JsValue::from_str("isSolflare")) {
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

        let req = SolflareRequest {
            method: "signAndSendTransaction".to_string(),
            params: SolflareRequestParams {
                transaction: tx_bs58,
            },
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
}

pub struct SolflareWalletAdapter {
    adapter: GenericWasmWalletAdapter<SolflareWallet>,
}

impl SolflareWalletAdapter {
    pub fn new() -> Result<Self> {
        Ok(Self {
            adapter: GenericWasmWalletAdapter::new(SolflareWallet)?,
        })
    }

    pub fn to_dyn_adapter(&self) -> Box<dyn BaseWalletAdapter> {
        Box::new(self.adapter.clone())
    }
}
