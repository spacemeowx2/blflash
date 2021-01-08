use std::{env, path::PathBuf};
use wasm_bindgen_webidl::{generate, Options};

fn main() {
    let crate_dir: PathBuf = env::var_os("CARGO_MANIFEST_DIR").unwrap().into();
    let out_dir: PathBuf = env::var_os("OUT_DIR").unwrap().into();
    generate(
        crate_dir.join("webidl").as_path(),
        out_dir.join("serial").as_path(),
        Options { features: false },
    )
    .unwrap();
}
