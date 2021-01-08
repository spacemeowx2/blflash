use blflash::{Boot2Opt, Connection, Error, FlashOpt};
use wasm_bindgen::prelude::*;

fn map_result(r: Result<(), Error>) -> JsValue {
    match r {
        Ok(_) => JsValue::UNDEFINED,
        Err(e) => JsValue::from_str(&e.to_string()),
    }
}

#[wasm_bindgen]
pub async fn flash(opt: JsValue) -> JsValue {
    let opt: FlashOpt = opt.into_serde().unwrap();
    let result = blflash::flash(opt).await;
    map_result(result)
}
