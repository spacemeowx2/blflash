use blflash::{Boot2Opt, Connection, Error, FlashOpt, DumpOpt, fs};
use wasm_bindgen::prelude::*;
use std::path::PathBuf;
mod utils;

fn map_result(r: Result<(), Error>) -> JsValue {
    match r {
        Ok(_) => JsValue::UNDEFINED,
        Err(e) => JsValue::from_str(&e.to_string()),
    }
}

#[wasm_bindgen]
pub async fn flash(opt: JsValue) -> JsValue {
    utils::set_panic_hook();

    let opt: FlashOpt = opt.into_serde().unwrap();
    let result = blflash::flash(opt).await;
    map_result(result)
}

#[wasm_bindgen]
pub async fn dump(opt: JsValue) -> JsValue {
    utils::set_panic_hook();

    let opt: DumpOpt = opt.into_serde().unwrap();
    let result = blflash::dump(opt).await;
    map_result(result)
}

struct FS;

#[wasm_bindgen]
impl FS {
    #[wasm_bindgen]
    pub fn read_file(path: String) -> Option<Vec<u8>> {
        fs::fs_read_file(PathBuf::from(path))
    }
    
    #[wasm_bindgen]
    pub fn write_file(path: String, content: Vec<u8>) {
        fs::fs_write_file(PathBuf::from(path), content)
    }
    
    #[wasm_bindgen]
    pub fn remove_file(path: String) {
        fs::fs_remove_file(PathBuf::from(path))
    }
}
