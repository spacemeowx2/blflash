#![cfg(target_arch = "wasm32")]

pub mod binding {
    include!(concat!(env!("OUT_DIR"), "/serial/mod.rs"));
}

use crate::Error;
use async_timer::oneshot::{Oneshot, Timer};
use futures::{
    channel::mpsc,
    io::{AsyncRead, AsyncWrite},
    ready,
    sink::Sink,
    stream::{BoxStream, IntoAsyncRead, Stream, StreamExt, TryStreamExt},
};
use gloo_events::EventListener;
use js_sys::Reflect;
use serial::SerialPortSettings;
use std::{
    io,
    pin::Pin,
    sync::Mutex,
    task::{Context, Poll},
    time::Duration,
};
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;
use wasm_streams::{
    readable::{IntoStream, ReadableStream},
    writable::{IntoSink, WritableStream},
};
use web_sys::{window, EventTarget};

#[wasm_bindgen]
extern "C" {
    // #[wasm_bindgen(extends=js_sys::Object, js_name=Serial , typescript_type="Serial")]
    // pub type Serial;
    // #[wasm_bindgen(catch, method, structural, js_class="Serial" , js_name=requestPort)]
    // pub fn requestPort(this: &Serial) -> Result<Promise, JsValue>;
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

type Item = io::Result<Vec<u8>>;

pub struct AsyncSerial {
    // receiver: IntoAsyncRead<mpsc::Receiver<Item>>,
    readable: IntoAsyncRead<BoxStream<'static, Item>>,
    writable: IntoSink<'static>,
    port: binding::SerialPort,
    signals: binding::SerialOutputSignals,
    timeout: Duration,
    writing: bool,

    open: Mutex<bool>,

    _on_connect: EventListener,
    _on_disconnect: EventListener,
}

impl AsyncSerial {
    pub async fn open(_port: &str) -> crate::Result<Self> {
        let window = window().ok_or(Error::WebError("Failed to get window"))?;
        let navigator = window
            .navigator()
            .dyn_into::<binding::Navigator>()
            .expect("navigator");
        let serial = navigator.serial();
        let port = Into::<JsFuture>::into(serial.request_port())
            .await
            .expect("Failed to await request_port")
            .dyn_into::<binding::SerialPort>()
            .expect("SerialPort");
        let target: JsValue = port.clone().into();
        let target: EventTarget = target.into();

        // let (sender, b) = mpsc::channel(128);
        // let (c, receiver) = mpsc::channel(128);

        let _on_connect = EventListener::new(&target, "connect", move |_event| log("on connect!"));
        let _on_disconnect =
            EventListener::new(&target, "disconnect", move |_event| log("on disconnect!"));

        let readable = Reflect::get(&port, &"readable".into()).expect("readable");
        let writable = Reflect::get(&port, &"writable".into()).expect("writable");

        let readable = ReadableStream::from_raw(readable.unchecked_into());
        let writable = WritableStream::from_raw(writable.unchecked_into());

        Ok(AsyncSerial {
            writable: writable.into_sink(),
            readable: readable.into_stream().boxed().into_async_read(),
            port,
            signals: binding::SerialOutputSignals::new(),
            timeout: Duration::from_secs(1),
            open: Mutex::new(false),
            writing: false,
            _on_connect,
            _on_disconnect,
        })
    }
    pub async fn set_rts(&mut self, level: bool) -> serial::Result<()> {
        self.signals.request_to_send(level);
        Into::<JsFuture>::into(self.port.set_signals_with_signals(&self.signals))
            .await
            .expect("Failed to set signals");
        Ok(())
    }
    pub async fn set_dtr(&mut self, level: bool) -> serial::Result<()> {
        self.signals.data_terminal_ready(level);
        Into::<JsFuture>::into(self.port.set_signals_with_signals(&self.signals))
            .await
            .expect("Failed to set signals");
        Ok(())
    }
    pub async fn sleep(&self, duration: Duration) {
        Timer::new(duration).await;
    }
    pub async fn set_timeout(&mut self, timeout: Duration) -> serial::Result<()> {
        self.timeout = timeout;
        Ok(())
    }
    pub async fn timeout(&self) -> Duration {
        self.timeout
    }
    pub async fn reconfigure(
        &mut self,
        setup: &dyn Fn(&mut dyn SerialPortSettings) -> serial::Result<()>,
    ) -> serial::Result<()> {
        // TODO
        Ok(())
    }
}

impl AsyncRead for AsyncSerial {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        AsyncRead::poll_read(Pin::new(&mut self.readable), cx, buf)
    }
}

impl AsyncWrite for AsyncSerial {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if !self.writing {
            self.writing = true;
            self.writable
                .start_send(Ok(Vec::from(buf)))
                .map_err(|_| Into::<io::Error>::into(io::ErrorKind::BrokenPipe))?
        }
        let r = ready!(self.writable.poll_ready(cx));
        r.map_err(|_| Into::<io::Error>::into(io::ErrorKind::BrokenPipe))?;
        self.writing = false;
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
