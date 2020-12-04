use std::fs::read;

use blflash::{Config, Flasher, Error, chip::bl602::{self, Bl602}, image::BootHeaderCfgFile};
use main_error::MainError;
use serial::BaudRate;
use env_logger::Env;
use structopt::StructOpt;
use std::path::PathBuf;

#[derive(StructOpt)]
struct FlashOpt {
    /// Serial port
    #[structopt(short, long)]
    port: String,
    /// Bin file
    #[structopt(parse(from_os_str))]
    image: PathBuf,
    /// Path to partition_cfg.toml, default to be partition/partition_cfg_2M.toml
    partition_cfg: Option<PathBuf>,
    /// Path to efuse_bootheader_cfg.conf
    boot_header_cfg: Option<PathBuf>,
    /// With boot2
    #[structopt(short, long)]
    without_boot2: bool,
}

#[derive(StructOpt)]
struct CheckOpt {
    /// Serial port
    #[structopt(short, long)]
    port: String,
    /// Bin file
    #[structopt(parse(from_os_str))]
    image: PathBuf,
}

#[derive(StructOpt)]
enum Opt {
    /// Flash image to serial
    Flash(FlashOpt),
    /// Check if the device's flash matches the image
    Check(CheckOpt),
}

fn flash(opt: FlashOpt) -> Result<(), Error> {
    let serial = serial::open(&opt.port)?;
    let chip = Bl602;

    if !opt.without_boot2 {
        let partition_cfg = opt
            .partition_cfg
            .map(read)
            .unwrap_or_else(|| Ok(bl602::DEFAULT_PARTITION_CFG.to_vec()))?;
        let boot_header_cfg = opt
            .boot_header_cfg
            .map(read)
            .unwrap_or_else(|| Ok(bl602::DEFAULT_BOOTHEADER_CFG.to_vec()))?;
        let partition_cfg = toml::from_slice(&partition_cfg)?;
        let BootHeaderCfgFile { boot_header_cfg } = toml::from_slice(&boot_header_cfg)?;

        let bin = read(&opt.image)?;
        let segments = chip.with_boot2(
            partition_cfg,
            boot_header_cfg,
            &bin,
        )?;
        let mut flasher = Flasher::connect(
            chip,
            serial,
            Some(BaudRate::Baud115200)
        )?;
    
        log::info!("Bootrom version: {}", flasher.boot_info().bootrom_version);
        log::trace!("Boot info: {:x?}", flasher.boot_info());

        flasher.load_segments(segments.into_iter())?;
    
        flasher.reset()?;
    } else {
        let mut flasher = Flasher::connect(
            chip,
            serial,
            Some(BaudRate::Baud115200)
        )?;
    
        log::info!("Bootrom version: {}", flasher.boot_info().bootrom_version);
        log::trace!("Boot info: {:x?}", flasher.boot_info());

        let input_bytes = read(&opt.image)?;
        flasher.load_elf_to_flash(&input_bytes)?;
    
        flasher.reset()?;
    }

    log::info!("Success");
    Ok(())
}

fn check(opt: CheckOpt) -> Result<(), Error> {
    let serial = serial::open(&opt.port)?;
    let mut flasher = Flasher::connect(
        Bl602,
        serial,
        Some(BaudRate::Baud115200),
    )?;

    log::info!("Bootrom version: {}", flasher.boot_info().bootrom_version);
    log::trace!("Boot info: {:x?}", flasher.boot_info());

    let input_bytes = read(&opt.image)?;
    flasher.check_elf_to_flash(&input_bytes)?;

    Ok(())
}

#[paw::main]
fn main(args: Opt) -> Result<(), MainError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("blflash=trace")).init();
    let _config = Config::load();
    
    match args {
        Opt::Flash(opt) => flash(opt)?,
        Opt::Check(opt) => check(opt)?,
    };


    Ok(())
}
