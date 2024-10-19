use std::{cell::RefCell, rc::Rc};

use anyhow::Result;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json::json;
use solana_sdk::hash::Hash;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::transaction::Transaction;
use solana_sdk::{commitment_config::CommitmentLevel, signature::Signature};
use wallet_adapter_base::{BaseWalletAdapter, TransactionOrVersionedTransaction};
use wallet_adapter_phantom::PhantomWalletAdapter;
use wallet_adapter_unsafe_burner::UnsafeBurnerWallet;
use wallet_adapter_web3::{Connection, SendTransactionOptions};
use wasm_bindgen::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
struct RpcResponse<T> {
    jsonrpc: String,
    result: T,
    id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct RpcRequest<T> {
    jsonrpc: String,
    method: String,
    params: T,
    id: u64,
}

impl<T> RpcRequest<T> {
    pub fn new(method: String, params: T) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method,
            params,
            id: 1,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLatestBlockhash {
    pub context: Context,
    pub value: Value,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub slot: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Value {
    pub blockhash: String,
    pub last_valid_block_height: i64,
}

//mod render;

const DEVNET_URL: &str = "https://api.devnet.solana.com";

struct WasmConnection {}

#[async_trait::async_trait(?Send)]
impl Connection for WasmConnection {
    async fn get_recent_blockhash(
        &self,
        commitment: Option<CommitmentLevel>,
        _min_context_slots: Option<u32>,
    ) -> Result<Hash> {
        let req = RpcRequest::new(
            "getLatestBlockhash".to_string(),
            json!([{"commitment": commitment.unwrap_or(CommitmentLevel::Processed)}]),
        );

        let resp: RpcResponse<GetLatestBlockhash> = Request::post(DEVNET_URL)
            .header("Content-Type", "application/json")
            .json(&req)?
            .send()
            .await?
            .json()
            .await?;

        console_log(format!("resp: {}", serde_json::to_string_pretty(&resp)?).as_str());

        Ok(resp.result.value.blockhash.parse()?)
    }

    async fn send_raw_transaction(
        &self,
        _raw_transaction: Vec<u8>,
        _options: Option<SendTransactionOptions>,
    ) -> Result<Signature> {
        console_log("||| send_raw_transaction |||");
        todo!()
    }
}

struct ButtonListeners {
    _connect: Closure<dyn FnMut()>,
    _disconnect: Closure<dyn FnMut()>,
    _send_tx: Closure<dyn FnMut()>,
}

thread_local! {
    static BUTTON_LISTENERS: RefCell<Option<ButtonListeners>> = RefCell::new(None);
    static WALLET_ADAPTER: RefCell<Option<Box<dyn BaseWalletAdapter>>> = RefCell::new(None);
}

use wasm_bindgen_futures::spawn_local;
use web_sys::{
    js_sys::wasm_bindgen,
    wasm_bindgen::{prelude::Closure, JsCast},
};

fn console_log(msg: &str) {
    web_sys::console::log_1(&msg.into());
}

pub fn register_disconnect_btn(wallet_adapter: &PhantomWalletAdapter) -> Closure<dyn FnMut()> {
    let window = web_sys::window().expect("global window does not exists");
    let document = window.document().expect("expecting a document on window");

    let wallet_adapter = wallet_adapter.clone();

    let on_disconnect_button_clicked = Closure::new(Box::new(move || {
        console_log("Disconnect button clicked");
        let wallet_adapter = wallet_adapter.clone();
        spawn_local(async move {
            console_log("Disconnecting wallet...");
            console_log(format!("ready state: {}", wallet_adapter.ready_state()).as_str());
            wallet_adapter.disconnect().await.unwrap();
        });
    }) as Box<dyn FnMut()>);

    document
        .get_element_by_id("disconnect-btn")
        .expect("should have a button on the page")
        .dyn_ref::<web_sys::HtmlElement>()
        .expect("#button-click-test be an `HtmlElement`")
        .set_onclick(Some(on_disconnect_button_clicked.as_ref().unchecked_ref()));

    on_disconnect_button_clicked
}

pub fn register_connect_btn(wallet_adapter: &PhantomWalletAdapter) -> Closure<dyn FnMut()> {
    let window = web_sys::window().expect("global window does not exists");
    let document = window.document().expect("expecting a document on window");

    let wallet_adapter = wallet_adapter.clone();

    let on_connect_button_clicked = Closure::new(Box::new(move || {
        console_log("Connect button clicked");
        let mut wallet_adapter = wallet_adapter.clone();
        spawn_local(async move {
            console_log("Connecting wallet...");
            console_log(format!("ready state: {}", wallet_adapter.ready_state()).as_str());
            wallet_adapter.connect().await.unwrap();
        });
    }) as Box<dyn FnMut()>);

    document
        .get_element_by_id("connect-btn")
        .expect("should have a button on the page")
        .dyn_ref::<web_sys::HtmlElement>()
        .expect("#button-click-test be an `HtmlElement`")
        .set_onclick(Some(on_connect_button_clicked.as_ref().unchecked_ref()));

    on_connect_button_clicked
}

pub fn register_send_tx_btn(wallet_adapter: &PhantomWalletAdapter) -> Closure<dyn FnMut()> {
    let window = web_sys::window().expect("global window does not exists");
    let document = window.document().expect("expecting a document on window");

    let wallet_adapter = wallet_adapter.clone();

    let on_send_tx_btn_clicked = Closure::new(Box::new(move || {
        console_log("Sign and send btn clicked");
        let wallet_adapter = wallet_adapter.clone();
        spawn_local(async move {
            console_log("sending tx");

            let public_key = wallet_adapter.public_key().unwrap();

            let idl_bytes = include_bytes!("../test_data/anchor_playground.json");
            let idl = anchor_lang_idl::convert::convert_idl(idl_bytes).unwrap();

            let program_id: Pubkey = idl.address.parse().unwrap();

            let data = idl.instructions[0].discriminator.clone();

            console_log(format!("program_id: {}", program_id).as_str());
            console_log(format!("data: {}", hex::encode(&data)).as_str());

            let instruction = Instruction::new_with_bytes(
                program_id,
                &data,
                vec![
                    AccountMeta::new(public_key, true),
                    AccountMeta::new(program_id, false),
                ],
            );

            let tx = Transaction::new_unsigned(solana_sdk::message::Message::new(
                &[instruction],
                Some(&public_key),
            ));

            let connection = WasmConnection {};

            match wallet_adapter
                .send_transaction(
                    TransactionOrVersionedTransaction::Transaction(tx),
                    &connection,
                    None,
                )
                .await
            {
                Ok(sig) => {
                    console_log(format!("tx_sig: {:?}", sig).as_str());
                }
                Err(e) => {
                    console_log(format!("error: {:?}", e).as_str());
                }
            };
        });
    }) as Box<dyn FnMut()>);

    document
        .get_element_by_id("send-tx-btn")
        .expect("should have a button on the page")
        .dyn_ref::<web_sys::HtmlElement>()
        .expect("#button-click-test be an `HtmlElement`")
        .set_onclick(Some(on_send_tx_btn_clicked.as_ref().unchecked_ref()));

    on_send_tx_btn_clicked
}

pub fn set_public_key(public_key: &str) {
    let window = web_sys::window().expect("global window does not exists");
    let document = window.document().expect("expecting a document on window");

    let public_key_element = document
        .get_element_by_id("public-key")
        .expect("should have a public key element on the page")
        .dyn_into::<web_sys::HtmlElement>()
        .expect("#public-key be an `HtmlElement`");

    public_key_element.set_inner_text(public_key);
}

fn window() -> web_sys::Window {
    web_sys::window().expect("no global `window` exists")
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    window()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK");
}

#[wasm_bindgen(main)]
pub fn main() {
    let phantom_wallet = PhantomWalletAdapter::new().unwrap();
    let unsafe_burner_wallet = UnsafeBurnerWallet::new();

    let _wallets: Vec<Box<dyn BaseWalletAdapter>> =
        vec![Box::new(phantom_wallet), Box::new(unsafe_burner_wallet)];

    let phantom_wallet = PhantomWalletAdapter::new().unwrap();

    BUTTON_LISTENERS.with(|button_listeners| {
        *button_listeners.borrow_mut() = Some(ButtonListeners {
            _connect: register_connect_btn(&phantom_wallet),
            _disconnect: register_disconnect_btn(&phantom_wallet),
            _send_tx: register_send_tx_btn(&phantom_wallet),
        });
    });

    let mut phantom_copy = phantom_wallet.clone();
    wasm_bindgen_futures::spawn_local(async move {
        phantom_copy.auto_connect().await.unwrap();
    });

    let phantom_copy = phantom_wallet.clone();

    wasm_bindgen_futures::spawn_local(async move {
        loop {
            if let Some(ev) = phantom_copy.event_emitter().recv().await {
                use wallet_adapter_base::WalletAdapterEvent::*;
                match ev {
                    Connect(pubkey) => {
                        console_log("Wallet connected");
                        console_log(&format!("is connected: {}", phantom_copy.connected()));
                        set_public_key(&pubkey.to_string());
                    }
                    Disconnect => {
                        console_log("Wallet disconnected");
                        set_public_key("");
                    }
                    Error(wallet_error) => {
                        console_log(format!("Wallet error: {:?}", wallet_error).as_str());
                    }
                    ReadyStateChange(wallet_ready_state) => {
                        console_log(
                            format!("Wallet ready state: {:?}", wallet_ready_state).as_str(),
                        );
                    }
                }
            }
        }
    });

    // render
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::new(move || {
        // if let Some(public_key) = phantom_wallet.public_key() {
        //     set_public_key(public_key.to_string().as_str());
        // }
        // if let Ok(t) = phantom_copy.rx.try_recv() {
        //     console_log(format!("Wallet event: {:?}", t).as_str());
        // }
        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());
}
