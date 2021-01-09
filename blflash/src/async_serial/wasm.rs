#![cfg(target_arch = "wasm32")]

pub mod binding {
    include!(concat!(env!("OUT_DIR"), "/serial/mod.rs"));
}

use crate::Error;
use futures::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite},
    ready,
    sink::SinkExt,
    stream::{IntoAsyncRead, LocalBoxStream, StreamExt, TryStreamExt},
};
use gloo_timers::future::TimeoutFuture;
use js_sys::{Reflect, Uint8Array};
use serial::SerialPortSettings;
use std::{
    convert::TryInto,
    io,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use wasm_streams::{
    readable::ReadableStream,
    writable::{IntoSink, WritableStream},
};
use web_sys::window;

type Item = io::Result<Vec<u8>>;

struct Settings<'a>(&'a mut binding::SerialOptions);
impl<'a> SerialPortSettings for Settings<'a> {
    fn baud_rate(&self) -> Option<serial::BaudRate> {
        todo!()
    }

    fn char_size(&self) -> Option<serial::CharSize> {
        todo!()
    }

    fn parity(&self) -> Option<serial::Parity> {
        todo!()
    }

    fn stop_bits(&self) -> Option<serial::StopBits> {
        todo!()
    }

    fn flow_control(&self) -> Option<serial::FlowControl> {
        todo!()
    }

    fn set_baud_rate(&mut self, baud_rate: serial::BaudRate) -> serial::Result<()> {
        log::trace!("set_baud_rate {:?}", baud_rate);
        self.0.baud_rate(baud_rate.speed() as u32);
        Ok(())
    }

    fn set_char_size(&mut self, char_size: serial::CharSize) {
        log::trace!("set_char_size {:?}", char_size);
        self.0.data_bits(match char_size {
            serial::CharSize::Bits5 => 5,
            serial::CharSize::Bits6 => 6,
            serial::CharSize::Bits7 => 7,
            serial::CharSize::Bits8 => 8,
        });
    }

    fn set_parity(&mut self, parity: serial::Parity) {
        log::trace!("set_parity {:?}", parity);
        use binding::ParityType;
        use serial::Parity;
        self.0.parity(match parity {
            Parity::ParityNone => ParityType::None,
            Parity::ParityEven => ParityType::Even,
            Parity::ParityOdd => ParityType::Odd,
        });
    }

    fn set_stop_bits(&mut self, stop_bits: serial::StopBits) {
        log::trace!("set_stop_bits {:?}", stop_bits);
        self.0.stop_bits(match stop_bits {
            serial::StopBits::Stop1 => 1,
            serial::StopBits::Stop2 => 2,
        });
    }

    fn set_flow_control(&mut self, flow_control: serial::FlowControl) {
        log::trace!("set_flow_control {:?}", flow_control);
        use binding::FlowControlType;
        use serial::FlowControl;
        self.0.flow_control(match flow_control {
            FlowControl::FlowNone => FlowControlType::None,
            FlowControl::FlowHardware => FlowControlType::Hardware,
            _ => unreachable!(),
        });
    }
}

pub struct AsyncSerial {
    readable: Option<IntoAsyncRead<LocalBoxStream<'static, Item>>>,
    writable: Option<IntoSink<'static>>,
    port: binding::SerialPort,
    signals: binding::SerialOutputSignals,
    timeout: Duration,
    option: binding::SerialOptions,
    // opened: Rc<RefCell<bool>>,
    // _on_connect: EventListener,
    // _on_disconnect: EventListener,
}

fn get_pipe(
    port: &JsValue,
) -> crate::Result<(
    Option<IntoAsyncRead<LocalBoxStream<'static, Item>>>,
    Option<IntoSink<'static>>,
)> {
    let readable = Reflect::get(&port, &"readable".into())
        .map_err(|_| Error::WebError("Failed to get readable"))?;
    let writable = Reflect::get(&port, &"writable".into())
        .map_err(|_| Error::WebError("Failed to get writable"))?;

    let readable = ReadableStream::from_raw(readable.unchecked_into());
    let writable = WritableStream::from_raw(writable.unchecked_into());

    let readable = readable
        .into_stream()
        .map(|item| {
            item.map(|v| v.unchecked_into::<Uint8Array>().to_vec())
                .map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))
        })
        .boxed_local()
        .into_async_read();

    Ok((Some(readable), Some(writable.into_sink())))
}

impl AsyncSerial {
    pub async fn open(_port: &str) -> crate::Result<Self> {
        let window = window().ok_or(Error::WebError("Failed to get window"))?;
        let navigator: JsValue = window.navigator().into();
        let navigator: binding::Navigator = navigator.into();

        let serial = navigator.serial();
        let port = JsFuture::from(serial.request_port())
            .await
            .map_err(|_| Error::WebError("Failed to await request_port"))?;

        // let target: EventTarget = port.clone().into();
        let port: binding::SerialPort = port.into();

        // make sure the port is closed
        if let Err(e) = JsFuture::from(port.close()).await {
            log::warn!("Failed to close port {:?}", e);
        }

        let option = binding::SerialOptions::new(115200);
        JsFuture::from(port.open(&option))
            .await
            .map_err(|_| Error::WebError("Failed to open serial"))?;

        // let opened = Rc::new(RefCell::new(true));
        // let _on_connect = {
        //     let open = open.clone();
        //     EventListener::new(&target, "connect", move |_event| {
        //         log("on connect!");
        //         open.replace(true);
        //     })
        // };
        // let _on_disconnect = {
        //     let open = open.clone();
        //     EventListener::new(&target, "disconnect", move |_event| {
        //         log("on disconnect!");
        //         open.replace(false);
        //     })
        // };

        let (readable, writable) = get_pipe(&port)?;

        Ok(AsyncSerial {
            readable,
            writable,
            port,
            signals: binding::SerialOutputSignals::new(),
            timeout: Duration::from_secs(1),
            // opened,
            option,
            // _on_connect,
            // _on_disconnect,
        })
    }
    pub async fn set_rts(&mut self, level: bool) -> serial::Result<()> {
        self.signals.request_to_send(level);
        JsFuture::from(self.port.set_signals_with_signals(&self.signals))
            .await
            .map_err(|_| serial::Error::new(serial::ErrorKind::NoDevice, "Failed to set_rts"))?;
        Ok(())
    }
    pub async fn set_dtr(&mut self, level: bool) -> serial::Result<()> {
        self.signals.data_terminal_ready(level);
        JsFuture::from(self.port.set_signals_with_signals(&self.signals))
            .await
            .map_err(|_| serial::Error::new(serial::ErrorKind::NoDevice, "Failed to set_dtr"))?;
        Ok(())
    }
    pub async fn sleep(&self, duration: Duration) {
        TimeoutFuture::new(duration.as_millis().try_into().unwrap()).await;
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
        let mut settings = Settings(&mut self.option);
        setup(&mut settings)?;

        self.readable.take();
        self.writable.take();
        // make sure the port is closed
        if let Err(e) = JsFuture::from(self.port.close()).await {
            log::warn!("Failed to close port {:?}", e);
        }

        JsFuture::from(self.port.open(&self.option))
            .await
            .map_err(|_| {
                serial::Error::new(serial::ErrorKind::NoDevice, "Failed to open serial")
            })?;

        let (readable, writable) = get_pipe(&self.port)
            .map_err(|_| serial::Error::new(serial::ErrorKind::NoDevice, "Failed to get pipe"))?;
        self.readable = readable;
        self.writable = writable;

        Ok(())
    }
}

impl AsyncRead for AsyncSerial {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        // self.readable.poll_read_unpin(cx, buf)
        AsyncRead::poll_read(Pin::new(&mut self.readable.as_mut().unwrap()), cx, buf)
    }
}

impl AsyncWrite for AsyncSerial {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let writable = self.writable.as_mut().unwrap();
        ready!(writable.poll_ready_unpin(cx))
            .map_err(|_| Into::<io::Error>::into(io::ErrorKind::BrokenPipe))?;

        let jsbuf: JsValue = Uint8Array::from(buf).into();
        writable
            .start_send_unpin(jsbuf)
            .map_err(|_| Into::<io::Error>::into(io::ErrorKind::BrokenPipe))?;
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.writable.as_mut().unwrap().poll_flush_unpin(cx))
            .map_err(|_| Into::<io::Error>::into(io::ErrorKind::BrokenPipe))
            .into()
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.writable.as_mut().unwrap().poll_close_unpin(cx))
            .map_err(|_| Into::<io::Error>::into(io::ErrorKind::BrokenPipe))
            .into()
    }
}
