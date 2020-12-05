use blflash::{
    chip::{
        bl602::{self, Bl602},
        Chip,
    },
    elf::{FirmwareImage, RomSegment},
    image::BootHeaderCfgFile,
    Config, Error, Flasher,
};
use env_logger::Env;
use main_error::MainError;
use serial::{BaudRate, SerialPort};
use std::path::PathBuf;
use std::{
    borrow::Cow,
    fs::{read, File},
};
use structopt::StructOpt;

#[derive(StructOpt)]
struct Connection {
    /// Serial port
    #[structopt(short, long)]
    port: String,
    /// Flash baud rate
    #[structopt(short, long, default_value = "1000000")]
    baud_rate: usize,
    /// Initial baud rate
    #[structopt(long, default_value = "115200")]
    initial_baud_rate: usize,
}

#[derive(StructOpt)]
struct Boot2Opt {
    /// Path to partition_cfg.toml, default to be partition/partition_cfg_2M.toml
    #[structopt(parse(from_os_str))]
    partition_cfg: Option<PathBuf>,
    /// Path to efuse_bootheader_cfg.conf
    #[structopt(parse(from_os_str))]
    boot_header_cfg: Option<PathBuf>,
    /// With boot2
    #[structopt(short, long)]
    without_boot2: bool,
}

#[derive(StructOpt)]
struct FlashOpt {
    #[structopt(flatten)]
    conn: Connection,
    /// Bin file
    #[structopt(parse(from_os_str))]
    image: PathBuf,
    /// Don't skip if hash matches
    #[structopt(short, long)]
    force: bool,
    #[structopt(flatten)]
    boot: Boot2Opt,
}

#[derive(StructOpt)]
struct CheckOpt {
    #[structopt(flatten)]
    conn: Connection,
    /// Bin file
    #[structopt(parse(from_os_str))]
    image: PathBuf,
    #[structopt(flatten)]
    boot: Boot2Opt,
}

#[derive(StructOpt)]
struct DumpOpt {
    #[structopt(flatten)]
    conn: Connection,
    /// Output file
    #[structopt(parse(from_os_str))]
    output: PathBuf,
    /// start address
    #[structopt(parse(try_from_str = parse_int::parse), default_value = "0")]
    start: u32,
    /// end address
    #[structopt(parse(try_from_str = parse_int::parse), default_value = "0x100000")]
    end: u32,
}

#[derive(StructOpt)]
enum Opt {
    /// Flash image to serial
    Flash(FlashOpt),
    /// Check if the device's flash matches the image
    Check(CheckOpt),
    /// Dump the whole flash to a file
    Dump(DumpOpt),
}

impl Connection {
    fn open_serial(&self) -> Result<impl SerialPort, Error> {
        let serial = serial::open(&self.port)?;
        Ok(serial)
    }
    fn create_flasher(&self, chip: impl Chip + 'static) -> Result<Flasher, Error> {
        let serial = self.open_serial()?;
        Flasher::connect(
            chip,
            serial,
            BaudRate::from_speed(self.initial_baud_rate),
            BaudRate::from_speed(self.baud_rate),
        )
    }
}

impl Boot2Opt {
    fn with_boot2<'a>(
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

        let segments = chip.with_boot2(partition_cfg, boot_header_cfg, image)?;

        Ok(segments)
    }
    fn make_segment<'a>(
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
    fn get_segments<'a>(
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

fn read_image<'a>(chip: &dyn Chip, image: &'a [u8]) -> Result<Cow<'a, [u8]>, Error> {
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

fn flash(opt: FlashOpt) -> Result<(), Error> {
    let chip = Bl602;
    let mut flasher = opt.conn.create_flasher(chip)?;
    log::info!("Bootrom version: {}", flasher.boot_info().bootrom_version);
    log::trace!("Boot info: {:x?}", flasher.boot_info());
    let image = read(&opt.image)?;
    let image = read_image(&chip, &image)?;

    let segments = opt.boot.get_segments(&chip, Vec::from(image))?;
    flasher.load_segments(opt.force, segments.into_iter())?;
    flasher.reset()?;

    log::info!("Success");

    Ok(())
}

fn check(opt: CheckOpt) -> Result<(), Error> {
    let chip = Bl602;
    let mut flasher = opt.conn.create_flasher(Bl602)?;
    log::info!("Bootrom version: {}", flasher.boot_info().bootrom_version);
    log::trace!("Boot info: {:x?}", flasher.boot_info());
    let image = read(&opt.image)?;
    let image = read_image(&chip, &image)?;

    let segments = opt.boot.get_segments(&chip, Vec::from(image))?;
    flasher.check_segments(segments.into_iter())?;

    Ok(())
}

fn dump(opt: DumpOpt) -> Result<(), Error> {
    let mut flasher = opt.conn.create_flasher(Bl602)?;

    log::info!("Bootrom version: {}", flasher.boot_info().bootrom_version);
    log::trace!("Boot info: {:x?}", flasher.boot_info());

    let mut output = File::create(opt.output)?;
    flasher.dump_flash(opt.start..opt.end, &mut output)?;

    log::info!("Success");

    Ok(())
}

#[paw::main]
fn main(args: Opt) -> Result<(), MainError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("blflash=trace"))
        .format_timestamp(None)
        .init();
    let _config = Config::load();

    match args {
        Opt::Flash(opt) => flash(opt)?,
        Opt::Check(opt) => check(opt)?,
        Opt::Dump(opt) => dump(opt)?,
    };

    Ok(())
}
