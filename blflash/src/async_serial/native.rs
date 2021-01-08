#![cfg(not(target_arch = "wasm32"))]

use futures::io::{AsyncRead, AsyncWrite};
use serial::{Result, SerialPort, SerialPortSettings};
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
    thread::sleep,
    time::Duration,
};

// Async wrapper for SerialPort
// Note: it's not really async. For native usage, async is not necessary.
pub struct AsyncSerial(Box<dyn SerialPort>);

impl AsyncSerial {
    pub async fn open(port: &str) -> crate::Result<Self> {
        Ok(AsyncSerial(Box::new(serial::open(port)?)))
    }
    // pub fn new(serial: impl SerialPort + 'static) -> Self {
    //     Self(Box::new(serial))
    // }
    pub async fn set_rts(&mut self, level: bool) -> Result<()> {
        self.0.set_rts(level)
    }
    pub async fn set_dtr(&mut self, level: bool) -> Result<()> {
        self.0.set_dtr(level)
    }
    pub async fn sleep(&self, duration: Duration) {
        sleep(duration)
    }
    pub async fn set_timeout(&mut self, timeout: Duration) -> Result<()> {
        self.0.set_timeout(timeout)
    }
    pub async fn timeout(&self) -> Duration {
        self.0.timeout()
    }
    pub async fn reconfigure(
        &mut self,
        setup: &dyn Fn(&mut dyn SerialPortSettings) -> Result<()>,
    ) -> Result<()> {
        self.0.reconfigure(setup)
    }
}

impl AsyncRead for AsyncSerial {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(self.0.read(buf))
    }
}

impl AsyncWrite for AsyncSerial {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(self.0.write(buf))
    }

    fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(self.0.flush())
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
