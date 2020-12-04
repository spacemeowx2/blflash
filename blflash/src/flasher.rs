use crate::{connection::{Connection, DEFAULT_BAUDRATE}, elf::RomSegment};
use crate::Error;
use crate::chip::Chip;
use crate::elf::FirmwareImage;
use serial::{BaudRate, SerialPort};
use std::{time::{Duration, Instant}, io::{Cursor, Read}};
use deku::prelude::*;
use indicatif::HumanBytes;
use sha2::{Sha256, Digest};
use std::thread::sleep;

pub struct Flasher {
    connection: Connection,
    boot_info: protocol::BootInfo,
    chip: Box<dyn Chip>,
}

impl Flasher {
    pub fn connect(
        chip: impl Chip + 'static,
        serial: impl SerialPort + 'static,
        speed: Option<BaudRate>,
    ) -> Result<Self, Error> {
        let mut flasher = Flasher {
            connection: Connection::new(serial),
            boot_info: protocol::BootInfo::default(),
            chip: Box::new(chip),
        };
        flasher.connection.set_baud(speed.unwrap_or(DEFAULT_BAUDRATE))?;
        flasher.start_connection()?;
        flasher.connection.set_timeout(Duration::from_secs(10))?;
        flasher.boot_info = flasher.get_boot_info()?;

        Ok(flasher)
    }

    pub fn boot_info(&self) -> &protocol::BootInfo {
        &self.boot_info
    }

    pub fn load_segments<'a>(&'a mut self, segments: impl Iterator<Item=RomSegment<'a>>) -> Result<(), Error> {
        self.load_eflash_loader()?;
        self.connection.set_baud(BaudRate::BaudOther(2_000_000))?;
        self.handshake()?;

        for segment in segments {
            log::info!("Erase flash addr: {:x} size: {}", segment.addr, segment.size());
            self.flash_erase(segment.addr, segment.addr + segment.size())?;
            let local_hash = Sha256::digest(&segment.data[0..segment.size() as usize]);

            let mut reader = Cursor::new(&segment.data);
            let mut cur = segment.addr;
            
            let start = Instant::now();
            log::info!("Program flash... {:x}", local_hash);
            loop {
                let size = self.flash_program(cur, &mut reader)?;
                // log::trace!("program {:x} {:x}", cur, size);
                cur += size;
                if size == 0 {
                    break
                }
            }
            let elapsed = start.elapsed();
            log::info!("Program done {:?} {}/s", elapsed, HumanBytes((segment.size() as f64 / elapsed.as_millis() as f64 * 1000.0) as u64));

            let sha256 = self.sha256_read(segment.addr, segment.size())?;
            if sha256 != &local_hash[..] {
                log::warn!("sha256 not match: {:x?} != {:x?}", sha256, local_hash);
            }
        }
        Ok(())
    }

    pub fn check_segments<'a>(&'a mut self, segments: impl Iterator<Item=RomSegment<'a>>) -> Result<(), Error> {
        self.load_eflash_loader()?;
        self.connection.set_baud(BaudRate::BaudOther(2_000_000))?;
        self.handshake()?;

        for segment in segments {
            let local_hash = Sha256::digest(&segment.data[0..segment.size() as usize]);

            let sha256 = self.sha256_read(segment.addr, segment.size())?;
            if sha256 != &local_hash[..] {
                log::warn!("{:x} sha256 not match: {:x?} != {:x?}", segment.addr, sha256, local_hash);
            } else {
                log::info!("{:x} sha256 match", segment.addr);
            }
        }
        Ok(())
    }

    pub fn load_elf_to_flash(&mut self, elf_data: &[u8]) -> Result<(), Error> {
        let image = FirmwareImage::from_data(elf_data).map_err(|_| Error::InvalidElf)?;
        let segs = image
            .segments()
            .filter_map(|segment| self.chip.get_flash_segment(segment))
            .collect::<Vec<_>>();

        self.load_segments(segs.into_iter())?;
        Ok(())
    }

    pub fn check_elf_to_flash(&mut self, elf_data: &[u8]) -> Result<(), Error> {
        let image = FirmwareImage::from_data(elf_data).map_err(|_| Error::InvalidElf)?;
        let segs = image
            .segments()
            .filter_map(|segment| self.chip.get_flash_segment(segment))
            .collect::<Vec<_>>();

        self.check_segments(segs.into_iter())?;
        Ok(())
    }

    pub fn load_eflash_loader(&mut self) -> Result<(), Error> {
        let input =  self.chip.get_eflash_loader().to_vec();
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
        sleep(Duration::from_millis(200));

        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        Ok(self.connection.reset()?)
    }

    fn sha256_read(&mut self, addr: u32, len: u32) -> Result<[u8; 32], Error> {
        let mut req = protocol::Sha256Read {
            addr,
            len,
        };
        req.update()?;
        self.connection.write_all(&req.to_bytes()?)?;
        self.connection.flush()?;

        let data = self.connection.read_response(34)?;
        let (_, data) = protocol::Sha256ReadResp::from_bytes((&data, 0))?;

        Ok(data.digest)
    }

    fn flash_program(&mut self, addr: u32, reader: &mut impl Read) -> Result<u32, Error> {
        let mut data = vec![0u8; 4000];
        let size = reader.read(&mut data)?;
        if size == 0 {
            return Ok(0)
        }
        data.truncate(size);
        let mut req = protocol::FlashProgram {
            addr,
            data,
            ..Default::default()
        };
        req.update()?;
        self.connection.write_all(&req.to_bytes()?)?;
        self.connection.flush()?;
        self.connection.read_response(0)?;

        Ok(size as u32)
    }

    fn flash_erase(&mut self, start: u32, end: u32) -> Result<(), Error> {
        let mut req = protocol::FlashErase {
            start,
            end,
        };
        req.update()?;
        self.connection.write_all(&req.to_bytes()?)?;
        self.connection.flush()?;
        self.connection.read_response(0)?;

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

    fn load_segment_data(&mut self, reader: &mut impl Read) -> Result<u32, Error> {
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

        Ok(size as u32)
    }

    pub fn get_boot_info(&mut self) -> Result<protocol::BootInfo, Error> {
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

    #[derive(Debug, DekuWrite, Default)]
    #[deku(magic = b"\x30\x00\x08\x00", endian = "little")]
    pub struct FlashErase {
        pub start: u32,
        pub end: u32,
    }

    #[derive(Debug, DekuWrite, Default)]
    #[deku(magic = b"\x31\x00", endian = "little")]
    pub struct FlashProgram {
        #[deku(update = "self.len()")]
        pub len: u16,
        pub addr: u32,
        pub data: Vec<u8>,
    }

    impl FlashProgram {
        fn len(&self) -> u16 {
            self.data.len() as u16 + 4
        }
    }

    #[derive(Debug, DekuWrite, Default)]
    #[deku(magic = b"\x3d\x00\x08\x00", endian = "little")]
    pub struct Sha256Read {
        pub addr: u32,
        pub len: u32,
    }

    #[derive(Debug, DekuRead)]
    #[deku(magic = b"\x20\x00")]
    pub struct Sha256ReadResp {
        pub digest: [u8; 32],
    }
}
