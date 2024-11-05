use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::transaction::Transaction;
use wallet_adapter_base::{BaseWalletAdapter, TransactionOrVersionedTransaction};
use wallet_adapter_wasm::generic_wallet::{GenericWasmWallet, GenericWasmWalletAdapter};
use wallet_adapter_wasm::util::reflect_get;
use wallet_binding::solana;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

mod wallet_binding {
    use js_sys::Object;

    use super::*;

    // BackpackRequestResponse
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen]
        #[derive(Clone, Debug)]
        pub type BackpackRequestResponse;

        #[wasm_bindgen(method, getter)]
        pub fn signature(this: &BackpackRequestResponse) -> Option<String>;
    }

    // BackpackError
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen]
        #[derive(Clone, Debug)]
        pub type BackpackError;

        #[wasm_bindgen(method, getter)]
        pub fn message(this: &BackpackError) -> Option<String>;
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

    // Backpack
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(thread_local, js_namespace = window, js_name = backpack)]
        pub static BACKPACK: Backpack;

        #[wasm_bindgen(extends = Object)]
        #[derive(Clone, Debug)]
        pub type Backpack;

        #[wasm_bindgen(method, catch)]
        pub async fn connect(
            this: &Backpack,
            options: &JsValue,
        ) -> std::result::Result<JsValue, BackpackError>;

        #[wasm_bindgen(method, getter, js_name = publicKey)]
        pub fn public_key(this: &Backpack) -> Pubkey;

        #[wasm_bindgen(method, getter, js_name = isConnected)]
        pub fn is_connected(this: &Backpack) -> bool;

        #[wasm_bindgen(method, catch)]
        pub fn disconnect(this: &Backpack) -> std::result::Result<(), BackpackError>;

        #[wasm_bindgen(method, js_name = signAndSendTransaction, catch)]
        pub async fn sign_and_send_transaction(
            this: &Backpack,
            tx: &JsValue,
            options: &JsValue,
        ) -> Result<BackpackRequestResponse, JsValue>;

        #[wasm_bindgen(method)]
        pub fn on(this: &Backpack, event: &str, cb: &js_sys::Function);
        #[wasm_bindgen(method)]
        pub fn off(this: &Backpack, event: &str, cb: &js_sys::Function);

        #[wasm_bindgen(method, getter, js_name = isBackpack)]
        pub fn is_backpack(this: &Backpack) -> bool;

    }

    pub fn solana() -> Backpack {
        BACKPACK.with(|backpack| backpack.clone())
    }
}

#[wasm_bindgen(inline_js = "
    export function convert_json_tx_to_tx(json_tx, serialize_fn) {
        json_tx.serialize = function() {
            return serialize_fn(this);
        }

        return json_tx;
    }
")]
extern "C" {
    fn convert_json_tx_to_tx(tx: JsValue, serialize_fn: &JsValue) -> JsValue;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackpackRequest {
    pub method: String,
    pub params: BackpackRequestParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackpackRequestParams {
    pub transaction: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BackpackWallet;

#[async_trait::async_trait(?Send)]
impl GenericWasmWallet for BackpackWallet {
    fn name(&self) -> String {
        "Backpack".into()
    }

    fn url(&self) -> String {
        "https://backpack.app".into()
    }

    fn icon(&self) -> String {
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAIAAAACACAYAAADDPmHLAAAACXBIWXMAAAsTAAALEwEAmpwYAAAAAXNSR0IArs4c6QAAAARnQU1BAACxjwv8YQUAAAbvSURBVHgB7Z1dUtxGEMf/LZH3fU0V4PUJQg4QVj5BnBOAT2BzAsMJAicwPoHJCRDrAxifgLVxVV73ObDqdEtsjKn4C8+0NDv9e7AxprRC85uvnp4RYYW5qKpxCVTcYKsgfiDfGjMwIsZIvh7d/lkmzAiYy5fzhultyZhdlagf1vU5VhjCiiGFXq01zYSJdqWgx/hB5AHN5I/6iuilyFBjxVgZAdqCZ34ORoVIqAzSOhxsvq6PsSIkL4A281LwL2IW/F1UhLKgRz/X9QyJUyBhuuae31gWviLjiPF1wxeX29vPkTjJtgAftrd3GHSMnmHw4eZ0uodESVKAoRT+kpQlSE6Ats/XZv/ONK5vZHC49+B1fYjESG4MUDKfYmCFr0ic4fmHqtpCYiQlgA66QsztIzFi5j+RGMl0AXebfgn0aOTuvGG8owIarZsXOj3ronlRuEYnn84CJLo4Lgi/QL/H/LHmy/RwI6GA0RoS4acFHi8kGieFXS/QhmijFfQXmH3uPy5lSkoLbIkYlfyzhuM4juM4juM4juMMj6TzATQ4JH9tlRqFk8BM2aV9RWHB9K5kzK/KLui0KqliSQmgBa4BIS54cpMD0OeawFye3jk19JdKkWq62OAFkEIfrTXNUxBV1okf38Ot3MGjlFqHwQrQZvQ22Cfw7xjg6t8XkZaBGzpKIXdwcAJojZeCP5SC30HipJBEOigBZLn3qdzSPlKr8V9hyEmkgxCgj8zefuD9jen0AAOidwE0i6ZhfjXgRI+gDK016DUjqE3ubPhNLoWvaDLJouHToaSP9SbA0DJ7LekyiviNPgP0TC9dQM6FfxeZ7eyuT6cv0RPmAmjTx11uXx/MiegEDd425cfcwWV+H4O3+uiO+pTAVIA2uMN8av6QiWr5TQ++JVlTc/tEiF3jOMScZGC43kME0VSA95PJhWXhM+Gt1Phn98nStZa1r9mB2SDQPqefjhayfnDfFG2J5882z84eynVM5u3thlONhRhj0gLc5PRfwAw62JjW+wjE5Xa1L0VkshO4kXt/EPDev4ZJCyBRvlcwggjHG4EfYHc9OoIBBWy3mEUX4H1V7Ur7ZvILaT8qy7FRduleF9jXc4RggOUWs/gtANs0nYquvMXaMaTXlQHlE1ggayLvf5OKY0DUMYDWfmpsBjZa+9enOmiLy+VkcmqxaNW2ZgX9GnsLXNQWoGj4KYzQ2g8LyG5WUDR4hshEE6CN+AFmg5lFiRMYcI0uKRQGyIAwegWKJkBjYO8tzq12C7efQ7CK2I00MomIxOsCiCcwQhaW3sEQ6W7sPi/yIDqKAHp8m2nIF7COoc9ghQw4NU8SkYgiQCmLKXCCUSziPc84XYBh83/DSiWR3qUo2tT4ONdGYDTub73cSzD/PNt0rojdQHAByoXxw0E7XfoFhsjnRduD+DnWIkkXXACJl1cwRoMmf3cbRaOjLRzDXnKZVj9GBIILUJBtbVzyj9HAU19AgR6I9VzDtwCgMXpAo2Yxp0v/Ybi49ennJtIFEPMY/TCKHTvv+aTSUQzBgwrQ92YHbQVi3UN3GAVZhrf/jzECE1SAq/7n4yOJ074KPSBcJoii598vxgwrqAByg70HZJZbr0JJ0G5XZz5Z1e1rYccA5TAicqEk0O5ECl/3LvYys7mLTLHHCEzS7wz6Esv3+nyYTF58rwha63XAl8PG1aCnhesWq6EdOcKM3WvmXRHh+Gvv/tNVTJlJPC4a3RVEK72+sCSZ4+J/FBVhTUS43J7gJqFjrnl33A3sxtCa3nAWhX6bbAT4hJugCsNZ2TGA8224AJnjAmSOC5A5LkDmuACZ4wJkjguQOS5A5rgAmeMCZI4LkDkuQOa4AJnjAmSOC5A5LkDmuACZ4wJkjguQOWEFYJvz85xwBBWgKM1P68oKKsI/36ACdC9nsDlWPTsIJ5t1Hfw01OBjgI1p/YwLegIibw0CwESz9gUYZ2d/wHEcx3Ecx3Ecx3Ecx3HuS5QjfdrXxTHv3JzEkd2xKwHR9xPNuKGjzdf1MSIQXAA9XUsuuw8nKPpK3PWzs+AvrgwqgP1LojOjoEf3fRv6Zy+JgBSLOGfaOx1NE/6o+rCrgeT9fWp4SljmuACZ4wJkjguQOS5A5rgAmeMCZI4LkDkuQOa4AJnjAmSOC5A5LkDmuACZ4wJkjguQOS5A5rgAmeMCZI4LkDkuQOa4AJnj5wRmTlABqHQBohKhggUVYAEEP8fO+UiMgziDCvCwrnU3aw0nOATMQu8LVIIPAq+JdAerdwWBaQ/fjEBwAaQVmMnN7sEJCB3EqP3tlRGJy6qqmPkFMcZw7sucmfZiHQ6hRBNgSXdaCHbA7KeFfBvz9pxlxtl1gcN2XBWRfwHK959XFRG6AgAAAABJRU5ErkJggg==".into()
    }

    fn is_correct_wallet(&self) -> bool {
        let window = web_sys::window().expect("no global `window` exists");

        if let Ok(backpack) = reflect_get(&window, &JsValue::from_str("backpack")) {
            tracing::debug!("backpack: {:?}", backpack);

            if let Ok(is_backpack) = reflect_get(&backpack, &JsValue::from("isBackpack")) {
                tracing::debug!("is_backpack: {:?}", is_backpack);

                return is_backpack.as_bool().unwrap_or(false);
            }
        }

        false
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
        tracing::debug!("backpack wallet connect");

        let result = solana()
            .connect(&JsValue::NULL)
            .await
            .map_err(|err| anyhow!("{:?}", err))?;

        tracing::debug!("{:?}", result);

        Ok(())
    }

    async fn sign_and_send_transaction(
        &self,
        transaction: TransactionOrVersionedTransaction,
    ) -> Result<solana_sdk::signature::Signature> {
        let TransactionOrVersionedTransaction::Transaction(tx) = transaction else {
            bail!("expected TransactionOrVersionedTransaction::Transaction");
        };

        let tx_as_value = serde_wasm_bindgen::to_value(&tx).map_err(|e| anyhow!("{:?}", e))?;
        tracing::info!("tx_value {:?}", tx_as_value);

        let closure = Closure::wrap(Box::new(move |tx: JsValue| {
            tracing::info!("{:?}", tx);
            let tx: Transaction = serde_wasm_bindgen::from_value(tx).unwrap();
            let tx_bytes = bincode::serialize(&tx).unwrap();
            tracing::info!("serialized");
            // disconnected code here

            tx_bytes
        }) as Box<dyn FnMut(JsValue) -> Vec<u8>>);

        let tx_as_value = convert_json_tx_to_tx(tx_as_value, closure.as_ref().unchecked_ref());

        closure.forget();

        let resp = solana()
            .sign_and_send_transaction(&tx_as_value, &JsValue::NULL)
            .await
            .map_err(|err| anyhow!("{:?}", err))?;

        let signature = resp.signature().context("signature not found")?;

        tracing::debug!("result: {}", signature);

        Ok(signature.parse()?)
    }
}

pub struct BackpackWalletAdapter {
    adapter: GenericWasmWalletAdapter<BackpackWallet>,
}

impl BackpackWalletAdapter {
    pub fn new() -> Result<Self> {
        Ok(Self {
            adapter: GenericWasmWalletAdapter::new(BackpackWallet)?,
        })
    }

    pub fn to_dyn_adapter(&self) -> Box<dyn BaseWalletAdapter> {
        Box::new(self.adapter.clone())
    }
}
