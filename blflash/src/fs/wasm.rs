#![cfg(target_arch = "wasm32")]

use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    io::{self, Cursor, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

static FS: Lazy<Mutex<HashMap<PathBuf, Vec<u8>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub struct File {
    path: PathBuf,
    buf: Option<Cursor<Vec<u8>>>,
}

impl File {
    pub fn create(path: PathBuf) -> io::Result<Self> {
        Ok(Self {
            path,
            buf: Some(Cursor::new(Vec::new())),
        })
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.as_mut().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buf.as_mut().unwrap().flush()
    }
}

impl Drop for File {
    fn drop(&mut self) {
        FS.lock()
            .unwrap()
            .insert(self.path.clone(), self.buf.take().unwrap().into_inner());
    }
}

pub fn read<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    FS.lock()
        .unwrap()
        .get(&path.as_ref().to_path_buf())
        .map(Clone::clone)
        .ok_or(io::ErrorKind::NotFound.into())
}

pub fn fs_read_file(path: PathBuf) -> Option<Vec<u8>> {
    FS.lock().unwrap().get(&path).map(Clone::clone)
}
pub fn fs_write_file(path: PathBuf, content: Vec<u8>) {
    FS.lock().unwrap().insert(path, content);
}
pub fn fs_remove_file(path: PathBuf) {
    FS.lock().unwrap().remove(&path);
}
