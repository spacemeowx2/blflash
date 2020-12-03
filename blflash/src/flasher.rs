use crate::connection::{Connection, DEFAULT_BAUDRATE};
use crate::Error;
use crate::chip::{Chip, Bl602};
use crate::image::BinReader;
use serial::{BaudRate, SerialPort};
use std::{time::{Duration, Instant}, io::{Cursor, Read}};
use deku::prelude::*;
use indicatif::{HumanBytes};

pub struct Flasher {
    connection: Connection,
    boot_info: protocol::BootInfo,
    chip: Box<dyn Chip>,
}

impl Flasher {
    pub fn connect(
        serial: impl SerialPort + 'static,
        speed: Option<BaudRate>,
    ) -> Result<Self, Error> {
        let mut flasher = Flasher {
            connection: Connection::new(serial),
            boot_info: protocol::BootInfo::default(),
            chip: Box::new(Bl602),
        };
        flasher.connection.set_baud(speed.unwrap_or(DEFAULT_BAUDRATE))?;
        flasher.start_connection()?;
        flasher.connection.set_timeout(Duration::from_secs(3))?;
        flasher.boot_info = flasher.get_boot_info()?;

        Ok(flasher)
    }

    pub fn boot_info(&self) -> &protocol::BootInfo {
        &self.boot_info
    }

    pub fn load_elf_to_flash(&mut self, _input: &[u8]) -> Result<(), Error> {
        let loader =  self.chip.get_eflash_loader().to_vec();
        self.load_bin(&loader)?;
        Ok(())
    }

    pub fn load_bin(&mut self, input: &[u8]) -> Result<(), Error> {
        let len = input.len();
        let mut reader = Cursor::new(input);
        self.load_boot_header(&mut reader)?;
        self.load_segment_header(&mut reader)?;

        let start = Instant::now();
        log::info!("Sending eflash_loader...");
        loop {
            let size = self.load_segment_data(&mut reader)?;
            if size == 0 {
                break
            }
        }
        let elapsed = start.elapsed();
        log::info!("Finished {:?} {}/s", elapsed, HumanBytes((len as f64 / elapsed.as_millis() as f64 * 1000.0) as u64));

        self.check_image()?;
        self.run_image()?;

        Ok(())
    }

    fn run_image(&mut self) -> Result<(), Error> {
        self.connection.write_all(protocol::RUN_IMAGE)?;
        self.connection.flush()?;
        self.connection.read_response(0)?;
        Ok(())
    }

    fn check_image(&mut self) -> Result<(), Error> {
        self.connection.write_all(protocol::CHECK_IMAGE)?;
        self.connection.flush()?;
        self.connection.read_response(0)?;
        Ok(())
    }

    fn load_boot_header(&mut self, reader: &mut impl Read) -> Result<(), Error> {
        let mut boot_header = vec![0u8; protocol::LOAD_BOOT_HEADER_LEN];
        reader.read_exact(&mut boot_header)?;
        let mut req = protocol::LoadBootHeader {
            boot_header,
            ..Default::default()
        };
        req.update()?;
        self.connection.write_all(&req.to_bytes()?)?;
        self.connection.flush()?;
        self.connection.read_response(0)?;

        Ok(())
    }

    fn load_segment_header(&mut self, reader: &mut impl Read) -> Result<(), Error> {
        let mut segment_header = vec![0u8; protocol::LOAD_SEGMENT_HEADER_LEN];
        reader.read_exact(&mut segment_header)?;
        let mut req = protocol::LoadSegmentHeader {
            segment_header,
            ..Default::default()
        };
        req.update()?;
        self.connection.write_all(&req.to_bytes()?)?;
        self.connection.flush()?;
        let resp = self.connection.read_response(18)?;

        if &resp[2..] != req.segment_header {
            log::warn!("Segment header not match req:{:x?} != resp:{:x?}", req.segment_header, &resp[2..])
        }

        Ok(())
    }

    fn load_segment_data(&mut self, reader: &mut impl Read) -> Result<usize, Error> {
        let mut segment_data = vec![0u8; 4000];
        let size = reader.read(&mut segment_data)?;
        if size == 0 {
            return Ok(0)
        }
        segment_data.truncate(size);
        let mut req = protocol::LoadSegmentData {
            segment_data,
            ..Default::default()
        };
        req.update()?;
        self.connection.write_all(&req.to_bytes()?)?;
        self.connection.flush()?;
        self.connection.read_response(0)?;

        Ok(size)
    }

    fn get_boot_info(&mut self) -> Result<protocol::BootInfo, Error> {
        self.connection.write_all(protocol::GET_BOOT_INFO)?;
        self.connection.flush()?;
        let data = self.connection.read_response(22)?;
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
    pub const CHECK_IMAGE: &[u8] = &[0x19, 0x00, 0x00, 0x00];
    pub const RUN_IMAGE: &[u8] = &[0x1a, 0x00, 0x00, 0x00];
    pub const LOAD_BOOT_HEADER_LEN: usize = 176;
    pub const LOAD_SEGMENT_HEADER_LEN: usize = 16;

    #[derive(Debug, DekuRead, Default)]
    #[deku(magic = b"\x14\x00")]
    pub struct BootInfo {
        pub bootrom_version: u32,
        pub otp_info: [u8; 16],
    }

    #[derive(Debug, DekuWrite, Default)]
    #[deku(magic = b"\x11\x00", endian = "little")]
    pub struct LoadBootHeader {
        #[deku(update = "self.boot_header.len()")]
        pub boot_header_len: u16,
        // length must be 176
        pub boot_header: Vec<u8>,
    }

    #[derive(Debug, DekuWrite, Default)]
    #[deku(magic = b"\x17\x00", endian = "little")]
    pub struct LoadSegmentHeader {
        #[deku(update = "self.segment_header.len()")]
        pub segment_header_len: u16,
        // length must be 16
        pub segment_header: Vec<u8>,
    }

    #[derive(Debug, DekuWrite, Default)]
    #[deku(magic = b"\x18\x00", endian = "little")]
    pub struct LoadSegmentData {
        #[deku(update = "self.segment_data.len()")]
        pub segment_data_len: u16,
        pub segment_data: Vec<u8>,
    }
}
