#![cfg(not(target_arch = "wasm32"))]

use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub fn read<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    fs::read(path)
}

pub use fs::File;

pub fn fs_read_file(path: PathBuf) -> Option<Vec<u8>> {
    unimplemented!("only avaliable on wasm")
}
pub fn fs_write_file(path: PathBuf, content: Vec<u8>) {
    unimplemented!("only avaliable on wasm")
}
pub fn fs_remove_file(path: PathBuf) {
    unimplemented!("only avaliable on wasm")
}
