use crate::connection::{Connection, DEFAULT_BAUDRATE};
use crate::Error;
use serial::{BaudRate, SerialPort};
use std::time::{Duration, Instant};
use deku::prelude::*;

pub struct Flasher {
    connection: Connection,
    bootrom_info: protocol::BootInfo,
}

impl Flasher {
    pub fn connect(
        serial: impl SerialPort + 'static,
        speed: Option<BaudRate>,
    ) -> Result<Self, Error> {
        let mut flasher = Flasher {
            connection: Connection::new(serial),
            bootrom_info: protocol::BootInfo::default(),
        };
        flasher.connection.set_baud(speed.unwrap_or(DEFAULT_BAUDRATE))?;
        flasher.start_connection()?;
        flasher.connection.set_timeout(Duration::from_secs(3))?;
        flasher.bootrom_info = flasher.get_boot_info()?;

        Ok(flasher)
    }

    fn get_boot_info(&mut self) -> Result<protocol::BootInfo, Error> {
        self.connection.write_all(protocol::GET_BOOT_INFO)?;
        let data = self.connection.read_response(1 + 1 + 4 + 16)?;
        let (_, data) = protocol::BootInfo::from_bytes((&data, 0))?;
        Ok(data)
    }

    fn handshake(&mut self) -> Result<(), Error> {
        self.connection
            .with_timeout(Duration::from_millis(200), |connection| {
                let len = connection.calc_duration_length(Duration::from_millis(5));
                log::trace!("5ms send count {}", len);
                let data: Vec<u8> = std::iter::repeat(0x55u8)
                    .take(len).collect();
                let start = Instant::now();
                connection.write_all(&data)?;
                connection.flush()?;
                log::trace!("handshake sent elapsed {:?}", start.elapsed());
                for _ in 0..10 {
                    if connection.read_response(0).is_ok() {
                        return Ok(())
                    }
                }
                Err(Error::Timeout)
            })
    }

    fn start_connection(&mut self) -> Result<(), Error> {
        log::info!("Start connection...");
        self.connection.reset_to_flash()?;
        for i in 1..=10 {
            self.connection.flush()?;
            if self.handshake().is_ok() {
                log::info!("Connection Succeed");
                return Ok(());
            } else {
                log::debug!("Retry {}", i);
            }
        }
        Err(Error::ConnectionFailed)
    }

}

mod protocol {
    use deku::prelude::*;

    pub const GET_BOOT_INFO: &[u8] = &[0x10, 0x00, 0x00, 0x00];

    #[derive(Debug, DekuRead, Default)]
    #[deku(magic = b"\x14\x00")]
    pub struct BootInfo {
        bootrom_version: u32,
        otp_info: [u8; 16],
    }
}
