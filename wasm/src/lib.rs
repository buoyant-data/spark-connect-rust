mod utils;

use tonic_web_wasm_client::Client;
use wasm_bindgen::prelude::*;

use spark_connect::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub async fn greet() {
    use web_sys::console::*;

    debug(&JsValue::from_str("Greetings!").into());
    let base_url = "http://localhost:15002";
    let mut spark = SparkSession::with_client(Client::new(base_url.into()));

    let _df = spark.sql("SELECT current_date();").await;
    //debug(&JsValue::from_str(&format!("response: {response:?}")).into());
    alert("Hello, wasm!");
}
