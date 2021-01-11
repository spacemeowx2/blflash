use blflash::{Error, FlashOpt, DumpOpt, fs};
use wasm_bindgen::prelude::*;
use std::path::PathBuf;
mod utils;

#[wasm_bindgen]
pub fn init_blflash() {
    utils::set_panic_hook();
    wasm_logger::init(wasm_logger::Config::new(log::Level::Trace));
}

#[wasm_bindgen]
pub async fn flash(opt: JsValue) -> Result<(), JsValue> {
    let opt: FlashOpt = opt.into_serde().map_err(|e| e.to_string())?;
    blflash::flash(opt).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[wasm_bindgen]
pub async fn dump(opt: JsValue) -> Result<(), JsValue> {
    let opt: DumpOpt = opt.into_serde().map_err(|e| e.to_string())?;
    blflash::dump(opt).await.map_err(|e| e.to_string())?;
    Ok(())
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
