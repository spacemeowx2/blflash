use crate::Error;
use crate::{async_serial::AsyncSerial, chip::Chip};
use crate::{connection::Connection, elf::RomSegment};
use indicatif::{HumanBytes, ProgressBar, ProgressStyle};
use serial::BaudRate;
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Read, Write},
    time::{Duration, Instant},
};
use std::{ops::Range, thread::sleep};

fn get_bar(len: u64) -> ProgressBar {
    let bar = ProgressBar::new(len);
    bar.set_style(
        ProgressStyle::default_bar()
            .template("  {wide_bar} {bytes}/{total_bytes} {bytes_per_sec} {eta}  ")
            .progress_chars("#>-"),
    );
    bar
}

pub struct Flasher {
    connection: Connection,
    boot_info: protocol::BootInfo,
    chip: Box<dyn Chip>,
    flash_speed: BaudRate,
}

impl Flasher {
    pub async fn connect(
        chip: impl Chip + 'static,
        serial: AsyncSerial,
        initial_speed: BaudRate,
        flash_speed: BaudRate,
    ) -> Result<Self, Error> {
        let mut flasher = Flasher {
            connection: Connection::new(serial),
            boot_info: protocol::BootInfo::default(),
            chip: Box::new(chip),
            flash_speed,
        };
        flasher.connection.set_baud(initial_speed).await?;
        flasher.start_connection().await?;
        flasher
            .connection
            .set_timeout(Duration::from_secs(10))
            .await?;
        flasher.boot_info = flasher.boot_rom().get_boot_info().await?;

        Ok(flasher)
    }

    pub fn into_inner(self) -> Connection {
        self.connection
    }

    pub fn boot_info(&self) -> &protocol::BootInfo {
        &self.boot_info
    }

    pub async fn load_segments<'a>(
        &'a mut self,
        force: bool,
        segments: impl Iterator<Item = RomSegment<'a>>,
    ) -> Result<(), Error> {
        self.load_eflash_loader().await?;

        for segment in segments {
            let local_hash = Sha256::digest(&segment.data[0..segment.size() as usize]);

            // skip segment if the contents are matched
            if !force {
                let sha256 = self
                    .eflash_loader()
                    .sha256_read(segment.addr, segment.size())
                    .await?;
                if sha256 == &local_hash[..] {
                    log::info!(
                        "Skip segment addr: {:x} size: {} sha256 matches",
                        segment.addr,
                        segment.size()
                    );
                    continue;
                }
            }

            log::info!(
                "Erase flash addr: {:x} size: {}",
                segment.addr,
                segment.size()
            );
            self.eflash_loader()
                .flash_erase(segment.addr, segment.addr + segment.size())
                .await?;

            let mut reader = Cursor::new(&segment.data);
            let mut cur = segment.addr;

            let start = Instant::now();
            log::info!("Program flash... {:x}", local_hash);
            let pb = get_bar(segment.size() as u64);
            loop {
                let size = self.eflash_loader().flash_program(cur, &mut reader).await?;
                // log::trace!("program {:x} {:x}", cur, size);
                cur += size;
                pb.inc(size as u64);
                if size == 0 {
                    break;
                }
            }
            pb.finish_and_clear();
            let elapsed = start.elapsed();
            log::info!(
                "Program done {:?} {}/s",
                elapsed,
                HumanBytes((segment.size() as f64 / elapsed.as_millis() as f64 * 1000.0) as u64)
            );

            let sha256 = self
                .eflash_loader()
                .sha256_read(segment.addr, segment.size())
                .await?;
            if sha256 != &local_hash[..] {
                log::warn!(
                    "sha256 not match: {} != {}",
                    hex::encode(sha256),
                    hex::encode(local_hash)
                );
            }
        }
        Ok(())
    }

    pub async fn check_segments<'a>(
        &'a mut self,
        segments: impl Iterator<Item = RomSegment<'a>>,
    ) -> Result<(), Error> {
        self.load_eflash_loader().await?;

        for segment in segments {
            let local_hash = Sha256::digest(&segment.data[0..segment.size() as usize]);

            let sha256 = self
                .eflash_loader()
                .sha256_read(segment.addr, segment.size())
                .await?;
            if sha256 != &local_hash[..] {
                log::warn!(
                    "{:x} sha256 not match: {} != {}",
                    segment.addr,
                    hex::encode(sha256),
                    hex::encode(local_hash)
                );
            } else {
                log::info!("{:x} sha256 match", segment.addr);
            }
        }
        Ok(())
    }

    pub async fn dump_flash(
        &mut self,
        range: Range<u32>,
        mut writer: impl Write,
    ) -> Result<(), Error> {
        self.load_eflash_loader().await?;

        const BLOCK_SIZE: usize = 4096;
        let mut cur = range.start;
        let pb = get_bar(range.len() as u64);
        while cur < range.end {
            let data = self
                .eflash_loader()
                .flash_read(cur, (range.end - cur).min(BLOCK_SIZE as u32))
                .await?;
            writer.write_all(&data)?;
            cur += data.len() as u32;
            pb.inc(data.len() as u64);
        }
        pb.finish_and_clear();

        Ok(())
    }

    pub async fn load_eflash_loader(&mut self) -> Result<(), Error> {
        let input = self.chip.get_eflash_loader().to_vec();
        let len = input.len();
        let mut reader = Cursor::new(input);
        self.boot_rom().load_boot_header(&mut reader).await?;
        self.boot_rom().load_segment_header(&mut reader).await?;

        let start = Instant::now();
        log::info!("Sending eflash_loader...");
        let pb = get_bar(len as u64);
        loop {
            let size = self.boot_rom().load_segment_data(&mut reader).await?;
            pb.inc(size as u64);
            if size == 0 {
                break;
            }
        }
        pb.finish_and_clear();
        let elapsed = start.elapsed();
        log::info!(
            "Finished {:?} {}/s",
            elapsed,
            HumanBytes((len as f64 / elapsed.as_millis() as f64 * 1000.0) as u64)
        );

        self.boot_rom().check_image().await?;
        self.boot_rom().run_image().await?;
        sleep(Duration::from_millis(500));
        self.connection.set_baud(self.flash_speed).await?;
        self.handshake().await?;

        log::info!("Entered eflash_loader");

        Ok(())
    }

    pub async fn reset(&mut self) -> Result<(), Error> {
        Ok(self.connection.reset().await?)
    }

    fn boot_rom(&mut self) -> BootRom {
        BootRom(&mut self.connection)
    }

    fn eflash_loader(&mut self) -> EflashLoader {
        EflashLoader(&mut self.connection)
    }

    async fn handshake(&mut self) -> Result<(), Error> {
        let connection = &mut self.connection;
        let old_timeout = connection.timeout().await;
        connection.set_timeout(Duration::from_millis(200)).await?;
        let result = async {
            let len = connection.calc_duration_length(Duration::from_millis(5));
            log::trace!("5ms send count {}", len);
            let data: Vec<u8> = std::iter::repeat(0x55u8).take(len).collect();
            let start = Instant::now();
            connection.write_all(&data).await?;
            connection.flush().await?;
            log::trace!("handshake sent elapsed {:?}", start.elapsed());
            sleep(Duration::from_millis(200));

            for _ in 0..5 {
                if connection.read_response(0).await.is_ok() {
                    return Ok(());
                }
            }

            Err(Error::Timeout)
        }
        .await;
        self.connection.set_timeout(old_timeout).await?;
        result
    }

    async fn start_connection(&mut self) -> Result<(), Error> {
        log::info!("Start connection...");
        self.connection.reset_to_flash().await?;
        for i in 1..=10 {
            self.connection.flush().await?;
            if self.handshake().await.is_ok() {
                log::info!("Connection Succeed");
                return Ok(());
            } else {
                log::debug!("Retry {}", i);
            }
        }
        Err(Error::ConnectionFailed)
    }
}

pub struct BootRom<'a>(&'a mut Connection);

impl<'a> BootRom<'a> {
    pub async fn run_image(&mut self) -> Result<(), Error> {
        self.0.command(protocol::RunImage {}).await?;
        Ok(())
    }

    pub async fn check_image(&mut self) -> Result<(), Error> {
        self.0.command(protocol::CheckImage {}).await?;
        Ok(())
    }

    pub async fn load_boot_header(&mut self, reader: &mut impl Read) -> Result<(), Error> {
        let mut boot_header = vec![0u8; protocol::LOAD_BOOT_HEADER_LEN];
        reader.read_exact(&mut boot_header)?;
        self.0
            .command(protocol::LoadBootHeader { boot_header })
            .await?;
        Ok(())
    }

    pub async fn load_segment_header(&mut self, reader: &mut impl Read) -> Result<(), Error> {
        let mut segment_header = vec![0u8; protocol::LOAD_SEGMENT_HEADER_LEN];
        reader.read_exact(&mut segment_header)?;

        let resp = self
            .0
            .command(protocol::LoadSegmentHeaderReq {
                segment_header: segment_header.clone(),
            })
            .await?;

        if resp.data != segment_header {
            log::warn!(
                "Segment header not match req:{:x?} != resp:{:x?}",
                segment_header,
                resp.data
            )
        }

        Ok(())
    }

    pub async fn load_segment_data(&mut self, reader: &mut impl Read) -> Result<u32, Error> {
        let mut segment_data = vec![0u8; 4000];
        let size = reader.read(&mut segment_data)?;
        if size == 0 {
            return Ok(0);
        }
        segment_data.truncate(size);

        self.0
            .command(protocol::LoadSegmentData { segment_data })
            .await?;

        Ok(size as u32)
    }

    pub async fn get_boot_info(&mut self) -> Result<protocol::BootInfo, Error> {
        self.0.command(protocol::BootInfoReq {}).await
    }
}

pub struct EflashLoader<'a>(&'a mut Connection);

impl<'a> EflashLoader<'a> {
    pub async fn sha256_read(&mut self, addr: u32, len: u32) -> Result<[u8; 32], Error> {
        Ok(self
            .0
            .command(protocol::Sha256Read { addr, len })
            .await?
            .digest)
    }

    pub async fn flash_read(&mut self, addr: u32, size: u32) -> Result<Vec<u8>, Error> {
        Ok(self
            .0
            .command(protocol::FlashRead { addr, size })
            .await?
            .data)
    }

    pub async fn flash_program(&mut self, addr: u32, reader: &mut impl Read) -> Result<u32, Error> {
        let mut data = vec![0u8; 4000];
        let size = reader.read(&mut data)?;
        if size == 0 {
            return Ok(0);
        }
        data.truncate(size);

        self.0
            .command(protocol::FlashProgram { addr, data })
            .await?;

        Ok(size as u32)
    }

    pub async fn flash_erase(&mut self, start: u32, end: u32) -> Result<(), Error> {
        self.0.command(protocol::FlashErase { start, end }).await?;

        Ok(())
    }
}

mod protocol {
    use crate::connection::{Command, Response};
    use deku::prelude::*;

    pub const LOAD_BOOT_HEADER_LEN: usize = 176;
    pub const LOAD_SEGMENT_HEADER_LEN: usize = 16;

    #[derive(Debug, DekuWrite, Default)]
    pub struct CheckImage {}
    impl_command!(0x19, CheckImage);

    #[derive(Debug, DekuWrite, Default)]
    pub struct RunImage {}
    impl_command!(0x1a, RunImage);

    #[derive(Debug, DekuWrite, Default)]
    pub struct BootInfoReq {}
    #[derive(Debug, DekuRead, Default)]
    pub struct BootInfo {
        pub len: u16,
        pub bootrom_version: u32,
        pub otp_info: [u8; 16],
    }
    impl_command!(0x10, BootInfoReq, BootInfo);

    #[derive(Debug, DekuWrite, Default)]
    pub struct LoadBootHeader {
        // length must be 176
        pub boot_header: Vec<u8>,
    }
    impl_command!(0x11, LoadBootHeader);

    #[derive(Debug, DekuWrite, Default)]
    pub struct LoadSegmentHeaderReq {
        // length must be 16
        pub segment_header: Vec<u8>,
    }
    #[derive(Debug, DekuRead)]
    pub struct LoadSegmentHeader {
        pub len: u16,
        #[deku(count = "len")]
        pub data: Vec<u8>,
    }
    impl_command!(0x17, LoadSegmentHeaderReq, LoadSegmentHeader);

    #[derive(Debug, DekuWrite, Default)]
    pub struct LoadSegmentData {
        pub segment_data: Vec<u8>,
    }
    impl_command!(0x18, LoadSegmentData);

    #[derive(Debug, DekuWrite, Default)]
    pub struct FlashErase {
        pub start: u32,
        pub end: u32,
    }
    impl_command!(0x30, FlashErase);

    #[derive(Debug, DekuWrite, Default)]
    pub struct FlashProgram {
        pub addr: u32,
        pub data: Vec<u8>,
    }
    impl_command!(0x31, FlashProgram);

    #[derive(Debug, DekuWrite, Default)]
    pub struct FlashRead {
        pub addr: u32,
        pub size: u32,
    }
    #[derive(Debug, DekuRead)]
    pub struct FlashReadResp {
        pub len: u16,
        #[deku(count = "len")]
        pub data: Vec<u8>,
    }
    impl_command!(0x32, FlashRead, FlashReadResp);

    #[derive(Debug, DekuWrite, Default)]
    pub struct Sha256Read {
        pub addr: u32,
        pub len: u32,
    }

    #[derive(Debug, DekuRead)]
    #[deku(magic = b"\x20\x00")]
    pub struct Sha256ReadResp {
        pub digest: [u8; 32],
    }
    impl_command!(0x3d, Sha256Read, Sha256ReadResp);
}
