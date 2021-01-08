pub mod chip;
mod connection;
pub mod elf;
mod error;
mod flasher;
pub mod image;
use async_serial::AsyncSerial;
use serde::Deserialize;
pub mod async_serial;
pub mod fs;

pub type Result<T, E = Error> = std::result::Result<T, E>;
pub use error::{Error, RomError};
pub use flasher::Flasher;

use crate::{
    chip::{
        bl602::{self, Bl602},
        Chip,
    },
    elf::{FirmwareImage, RomSegment},
    image::BootHeaderCfgFile,
};
use fs::{read, File};
use serial::{BaudRate, CharSize, FlowControl, Parity, SerialPortSettings, StopBits};
use std::{borrow::Cow, path::PathBuf};
use structopt::StructOpt;

#[derive(StructOpt, Deserialize)]
pub struct Connection {
    /// Serial port
    #[structopt(short, long)]
    pub port: String,
    /// Flash baud rate
    #[structopt(short, long, default_value = "1000000")]
    pub baud_rate: usize,
    /// Initial baud rate
    #[structopt(long, default_value = "115200")]
    pub initial_baud_rate: usize,
}

#[derive(StructOpt, Deserialize, Default)]
pub struct Boot2Opt {
    /// Path to partition_cfg.toml, default to be partition/partition_cfg_2M.toml
    #[structopt(long, parse(from_os_str))]
    pub partition_cfg: Option<PathBuf>,
    /// Path to efuse_bootheader_cfg.conf
    #[structopt(long, parse(from_os_str))]
    pub boot_header_cfg: Option<PathBuf>,
    /// Path to ro_params.dtb
    #[structopt(long, parse(from_os_str))]
    pub dtb: Option<PathBuf>,
    /// Without boot2
    #[structopt(short, long)]
    #[serde(default)]
    pub without_boot2: bool,
}

#[derive(StructOpt, Deserialize)]
pub struct FlashOpt {
    #[structopt(flatten)]
    #[serde(flatten)]
    pub conn: Connection,
    /// Bin file
    #[structopt(parse(from_os_str))]
    pub image: PathBuf,
    /// Don't skip if hash matches
    #[structopt(short, long)]
    #[serde(default)]
    pub force: bool,
    #[structopt(flatten)]
    #[serde(default, flatten)]
    pub boot: Boot2Opt,
}

#[derive(StructOpt, Deserialize)]
pub struct CheckOpt {
    #[structopt(flatten)]
    #[serde(flatten)]
    pub conn: Connection,
    /// Bin file
    #[structopt(parse(from_os_str))]
    pub image: PathBuf,
    #[structopt(flatten)]
    pub boot: Boot2Opt,
}

#[derive(StructOpt, Deserialize)]
pub struct DumpOpt {
    #[structopt(flatten)]
    #[serde(flatten)]
    pub conn: Connection,
    /// Output file
    #[structopt(parse(from_os_str))]
    pub output: PathBuf,
    /// start address
    #[structopt(parse(try_from_str = parse_int::parse), default_value = "0")]
    pub start: u32,
    /// end address
    #[structopt(parse(try_from_str = parse_int::parse), default_value = "0x100000")]
    pub end: u32,
}

#[derive(StructOpt)]
pub enum Opt {
    /// Flash image to serial
    Flash(FlashOpt),
    /// Check if the device's flash matches the image
    Check(CheckOpt),
    /// Dump the whole flash to a file
    Dump(DumpOpt),
}

impl Connection {
    pub async fn open_serial(&self) -> Result<AsyncSerial, Error> {
        let mut serial = AsyncSerial::open(&self.port).await?;
        serial
            .reconfigure(&|setup: &mut dyn SerialPortSettings| {
                setup.set_char_size(CharSize::Bits8);
                setup.set_stop_bits(StopBits::Stop1);
                setup.set_parity(Parity::ParityNone);
                setup.set_flow_control(FlowControl::FlowNone);
                Ok(())
            })
            .await?;
        Ok(serial)
    }
    pub async fn create_flasher(&self, chip: impl Chip + 'static) -> Result<Flasher, Error> {
        let serial = self.open_serial().await?;
        Flasher::connect(
            chip,
            serial,
            BaudRate::from_speed(self.initial_baud_rate),
            BaudRate::from_speed(self.baud_rate),
        )
        .await
    }
}

impl Boot2Opt {
    pub fn with_boot2<'a>(
        self,
        chip: &'a dyn Chip,
        image: &[u8],
    ) -> Result<Vec<RomSegment<'a>>, Error> {
        let partition_cfg = self
            .partition_cfg
            .map(read)
            .unwrap_or_else(|| Ok(bl602::DEFAULT_PARTITION_CFG.to_vec()))?;
        let boot_header_cfg = self
            .boot_header_cfg
            .map(read)
            .unwrap_or_else(|| Ok(bl602::DEFAULT_BOOTHEADER_CFG.to_vec()))?;
        let partition_cfg = toml::from_slice(&partition_cfg)?;
        let BootHeaderCfgFile { boot_header_cfg } = toml::from_slice(&boot_header_cfg)?;
        let ro_params = self
            .dtb
            .map(read)
            .unwrap_or_else(|| Ok(bl602::RO_PARAMS.to_vec()))?;

        let segments = chip.with_boot2(partition_cfg, boot_header_cfg, ro_params, image)?;

        Ok(segments)
    }
    pub fn make_segment<'a>(
        self,
        _chip: &'a dyn Chip,
        image: Vec<u8>,
    ) -> Result<RomSegment<'a>, Error> {
        let boot_header_cfg = self
            .boot_header_cfg
            .map(read)
            .unwrap_or_else(|| Ok(bl602::DEFAULT_BOOTHEADER_CFG.to_vec()))?;
        let BootHeaderCfgFile {
            mut boot_header_cfg,
        } = toml::from_slice(&boot_header_cfg)?;
        let img = boot_header_cfg.make_image(0x2000, image)?;

        Ok(RomSegment::from_vec(0x0, img))
    }
    pub fn get_segments<'a>(
        self,
        chip: &'a dyn Chip,
        image: Vec<u8>,
    ) -> Result<Vec<RomSegment<'a>>, Error> {
        Ok(if self.without_boot2 {
            vec![self.make_segment(chip, Vec::from(image))?]
        } else {
            self.with_boot2(chip, &image)?
        })
    }
}

pub fn read_image<'a>(chip: &dyn Chip, image: &'a [u8]) -> Result<Cow<'a, [u8]>, Error> {
    Ok(if image[0..4] == [0x7f, 0x45, 0x4c, 0x46] {
        log::trace!("Detect ELF");
        // ELF
        let firmware_image = FirmwareImage::from_data(image).map_err(|_| Error::InvalidElf)?;
        Cow::Owned(firmware_image.to_flash_bin(chip))
    } else {
        // bin
        Cow::Borrowed(image)
    })
}

pub async fn flash(opt: FlashOpt) -> Result<(), Error> {
    let chip = Bl602;
    let image = read(&opt.image)?;
    let image = read_image(&chip, &image)?;

    let mut flasher = opt.conn.create_flasher(chip).await?;
    log::info!("Bootrom version: {}", flasher.boot_info().bootrom_version);
    log::trace!("Boot info: {:x?}", flasher.boot_info());

    let segments = opt.boot.get_segments(&chip, Vec::from(image))?;
    flasher
        .load_segments(opt.force, segments.into_iter())
        .await?;
    flasher.reset().await?;

    log::info!("Success");

    Ok(())
}

pub async fn check(opt: CheckOpt) -> Result<(), Error> {
    let chip = Bl602;
    let image = read(&opt.image)?;
    let image = read_image(&chip, &image)?;

    let mut flasher = opt.conn.create_flasher(Bl602).await?;
    log::info!("Bootrom version: {}", flasher.boot_info().bootrom_version);
    log::trace!("Boot info: {:x?}", flasher.boot_info());

    let segments = opt.boot.get_segments(&chip, Vec::from(image))?;
    flasher.check_segments(segments.into_iter()).await?;

    Ok(())
}

pub async fn dump(opt: DumpOpt) -> Result<(), Error> {
    let mut output = File::create(opt.output)?;
    let mut flasher = opt.conn.create_flasher(Bl602).await?;

    log::info!("Bootrom version: {}", flasher.boot_info().bootrom_version);
    log::trace!("Boot info: {:x?}", flasher.boot_info());

    flasher.dump_flash(opt.start..opt.end, &mut output).await?;

    log::info!("Success");

    Ok(())
}
