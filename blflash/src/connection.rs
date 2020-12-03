use std::io::{Read, Write, Cursor};
use std::thread::sleep;
use std::time::Duration;
use crate::{Error, RomError};
use byteorder::{ReadBytesExt, LittleEndian};

use serial::{BaudRate, SerialPort, SerialPortSettings};

pub const DEFAULT_BAUDRATE: BaudRate = BaudRate::Baud115200;

pub struct Connection {
    serial: Box<dyn SerialPort>,
    baud_rate: BaudRate,
}

impl Connection {
    pub fn new(serial: impl SerialPort + 'static) -> Self {
        Connection {
            serial: Box::new(serial),
            baud_rate: DEFAULT_BAUDRATE,
        }
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
        self.baud_rate = speed;
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
        log::trace!("read_response {}", String::from_utf8_lossy(&resp).to_string());
        match &resp[0..2] {
            // OK
            [0x4f, 0x4b] => {
                if len > 0 {
                    self.read_exact(len)
                } else {
                    Ok(vec![])
                }
            },
            // FL
            [0x46, 0x4c] => {
                let code = self.read_exact(2)?;
                let mut reader = Cursor::new(code);
                let code = reader.read_u16::<LittleEndian>()?;
                Err(Error::RomError(RomError::from(code)))
            },
            _ => Err(Error::RespError)
        }
    }

    pub fn calc_duration_length(&mut self, duration: Duration) -> usize {
        self.baud_rate.speed() / 10 / 1000 * (duration.as_millis() as usize)
    }

    pub fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        Ok(self.serial.write_all(buf)?)
    }

    pub fn flush(&mut self) -> Result<(), Error> {
        Ok(self.serial.flush()?)
    }
}
