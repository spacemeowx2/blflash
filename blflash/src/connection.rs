#![macro_use]

use crate::{Error, RomError};
use byteorder::{ByteOrder, LittleEndian, ReadBytesExt, WriteBytesExt};
use deku::prelude::*;
use std::io::{Cursor, Read, Write};
use std::thread::sleep;
use std::time::Duration;

use serial::{BaudRate, SerialPort, SerialPortSettings};

pub const DEFAULT_BAUDRATE: BaudRate = BaudRate::Baud115200;

macro_rules! impl_command(
    ($id: expr, $t:ty, $r:ty) => (
        impl Command for $t {
            type Response = $r;

            const CMD_ID: u8 = $id;
        }
        impl Response for $r {}
    );
    ($id: expr, $t:ty) => (
        impl Command for $t {
            type Response = crate::connection::NoResponsePayload;

            const CMD_ID: u8 = $id;
        }
    );
);

#[derive(DekuRead)]
pub struct NoResponsePayload {}

impl Response for NoResponsePayload {
    fn no_response_payload() -> Option<Self> {
        Some(Self {})
    }
}

pub trait Response: for<'a> DekuContainerRead<'a> + Sized {
    fn from_payload(input: &[u8]) -> Result<Self, DekuError> {
        // We don't care about the lifetime 'a, as we only check the bit offset,
        // and don't hold onto the borrow on `input`.
        let (_, r) = DekuContainerRead::from_bytes((input, 0))?;
        Ok(r)
    }
    fn no_response_payload() -> Option<Self> {
        None
    }
}

pub trait Command: DekuContainerWrite {
    type Response: Response;
    const CMD_ID: u8;
    fn checksum(&self) -> u8 {
        0
    }
}

pub struct Connection {
    serial: Box<dyn SerialPort>,
    baud_rate: Option<BaudRate>,
}

impl Connection {
    pub fn new(serial: impl SerialPort + 'static) -> Self {
        Connection {
            serial: Box::new(serial),
            baud_rate: None,
        }
    }

    pub fn into_inner(self) -> Box<dyn SerialPort> {
        self.serial
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        self.serial.set_rts(false)?;
        sleep(Duration::from_millis(50));
        self.serial.set_dtr(true)?;
        sleep(Duration::from_millis(50));
        self.serial.set_dtr(false)?;
        sleep(Duration::from_millis(50));

        Ok(())
    }

    pub fn reset_to_flash(&mut self) -> Result<(), Error> {
        self.serial.set_rts(true)?;
        sleep(Duration::from_millis(50));
        self.serial.set_dtr(true)?;
        sleep(Duration::from_millis(50));
        self.serial.set_dtr(false)?;
        sleep(Duration::from_millis(50));
        self.serial.set_rts(false)?;
        sleep(Duration::from_millis(50));

        Ok(())
    }

    pub fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        self.serial.set_timeout(timeout)?;
        Ok(())
    }

    pub fn set_baud(&mut self, speed: BaudRate) -> Result<(), Error> {
        self.baud_rate = Some(speed);
        self.serial
            .reconfigure(&|setup: &mut dyn SerialPortSettings| setup.set_baud_rate(speed))?;
        Ok(())
    }

    pub fn with_timeout<T, F: FnMut(&mut Connection) -> Result<T, Error>>(
        &mut self,
        timeout: Duration,
        mut f: F,
    ) -> Result<T, Error> {
        let old_timeout = self.serial.timeout();
        self.serial.set_timeout(timeout)?;
        let result = f(self);
        self.serial.set_timeout(old_timeout)?;
        result
    }

    fn read_exact(&mut self, len: usize) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0u8; len];
        self.serial.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub fn read_response(&mut self, len: usize) -> Result<Vec<u8>, Error> {
        let resp = self.read_exact(2)?;
        match &resp[0..2] {
            // OK
            [0x4f, 0x4b] => {
                if len > 0 {
                    self.read_exact(len)
                } else {
                    Ok(vec![])
                }
            }
            // FL
            [0x46, 0x4c] => {
                let code = self.read_exact(2)?;
                let mut reader = Cursor::new(code);
                let code = reader.read_u16::<LittleEndian>()?;
                Err(Error::RomError(RomError::from(code)))
            }
            e => {
                log::trace!("read_response err: {:x?}", e);
                Err(Error::RespError)
            }
        }
    }

    pub fn calc_duration_length(&mut self, duration: Duration) -> usize {
        self.baud_rate.unwrap_or(DEFAULT_BAUDRATE).speed() / 10 / 1000
            * (duration.as_millis() as usize)
    }

    pub fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        Ok(self.serial.write_all(buf)?)
    }

    pub fn flush(&mut self) -> Result<(), Error> {
        Ok(self.serial.flush()?)
    }

    pub fn command<C: Command>(&mut self, command: C) -> Result<C::Response, Error> {
        let req = self.to_cmd(command)?;
        self.write_all(&req)?;
        self.flush()?;

        Ok(if let Some(resp) = C::Response::no_response_payload() {
            self.read_response(0)?;
            resp
        } else {
            let len = LittleEndian::read_u16(&self.read_response(2)?);
            let buf = Vec::new();
            let mut writer = Cursor::new(buf);
            writer.write_u16::<LittleEndian>(len)?;
            writer.write_all(&self.read_exact(len as usize)?)?;
            C::Response::from_payload(&writer.into_inner())?
        })
    }

    fn to_cmd<C: Command>(&self, command: C) -> Result<Vec<u8>, Error> {
        let data = Vec::new();
        let mut writer = Cursor::new(data);
        let body = command.to_bytes()?;
        let len = body.len() as u16;

        writer.write_u8(C::CMD_ID)?;
        writer.write_u8(command.checksum())?;
        writer.write_u16::<LittleEndian>(len)?;
        writer.write_all(&body)?;

        Ok(writer.into_inner())
    }
}
