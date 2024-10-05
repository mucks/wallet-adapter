use std::{cell::RefCell, rc::Rc};

use wallet_adapter_base::BaseWalletAdapter;
use wallet_adapter_phantom::PhantomWalletAdapter;
use wasm_bindgen::prelude::*;

//mod render;

struct ButtonListeners {
    connect: Closure<dyn FnMut()>,
}

thread_local! {
    static BUTTON_LISTENERS: RefCell<Option<ButtonListeners>> = RefCell::new(None);
    static WALLET_ADAPTER: RefCell<Option<PhantomWalletAdapter>> = RefCell::new(None);
}

use wasm_bindgen_futures::spawn_local;
use web_sys::{
    js_sys::wasm_bindgen,
    wasm_bindgen::{prelude::Closure, JsCast},
};

fn console_log(msg: &str) {
    web_sys::console::log_1(&msg.into());
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

    let mut phantom_copy = phantom_wallet.clone();
    wasm_bindgen_futures::spawn_local(async move {
        phantom_copy.auto_connect().await.unwrap();
    });

    BUTTON_LISTENERS.with(|button_listeners| {
        *button_listeners.borrow_mut() = Some(ButtonListeners {
            connect: register_connect_btn(&phantom_wallet),
        });
    });

    // render
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::new(move || {
        if let Some(public_key) = phantom_wallet.public_key() {
            set_public_key(public_key.to_string().as_str());
        }
        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());
}
