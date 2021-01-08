mod native;
pub mod wasm;

#[cfg(target_arch = "wasm32")]
pub use wasm::AsyncSerial;

#[cfg(not(target_arch = "wasm32"))]
pub use native::AsyncSerial;
