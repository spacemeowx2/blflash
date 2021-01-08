#![cfg(target_arch = "wasm32")]

use crate::Error;
use futures::io::{AsyncRead, AsyncWrite};
use js_sys::Reflect;
use serial::{Result, SerialPortSettings};
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
    thread::sleep,
    time::Duration,
};
use wasm_bindgen::prelude::*;
use web_sys::window;

pub struct AsyncSerial(JsValue);

impl AsyncSerial {
    pub async fn open(port: &str) -> crate::Result<Self> {
        let window = window().ok_or(Error::WebError("Failed to get window"))?;
        let serial = Reflect::get(window.navigator().as_ref(), &"serial".into())
            .map_err(|_| Error::WebError("Failed to get serial from navigator"))?;
        if serial.is_undefined() {
            return Err(Error::WebError("serial is not supported on your browser"));
        }

        todo!()
    }
    pub async fn set_rts(&mut self, level: bool) -> Result<()> {
        todo!()
    }
    pub async fn set_dtr(&mut self, level: bool) -> Result<()> {
        todo!()
    }
    pub async fn sleep(&self, duration: Duration) {
        sleep(duration)
    }
    pub async fn set_timeout(&mut self, timeout: Duration) -> Result<()> {
        todo!()
    }
    pub async fn timeout(&self) -> Duration {
        todo!()
    }
    pub async fn reconfigure(
        &mut self,
        setup: &dyn Fn(&mut dyn SerialPortSettings) -> Result<()>,
    ) -> Result<()> {
        todo!()
    }
}

impl AsyncRead for AsyncSerial {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        todo!()
    }
}

impl AsyncWrite for AsyncSerial {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        todo!()
    }

    fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        todo!()
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        todo!()
    }
}
