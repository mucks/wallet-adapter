use anyhow::{anyhow, Result};
use wasm_bindgen::JsValue;

// TODO: improve this function
pub async fn sleep_ms(millis: i32) {
    let mut cb = |resolve: js_sys::Function, _reject: js_sys::Function| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, millis)
            .expect("Failed to call set_timeout");
    };
    let p = js_sys::Promise::new(&mut cb);
    wasm_bindgen_futures::JsFuture::from(p).await.unwrap();
}

pub fn reflect_get(target: &JsValue, key: &JsValue) -> Result<JsValue> {
    let result = js_sys::Reflect::get(target, key).map_err(|e| anyhow!("{:?}", e))?;
    Ok(result)
}
