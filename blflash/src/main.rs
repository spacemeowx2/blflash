use std::fs::read;

use blflash::{Config, Flasher, Error};
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
    let mut flasher = Flasher::connect(serial, Some(BaudRate::Baud115200))?;

    log::info!("Bootrom version: {}", flasher.boot_info().bootrom_version);
    log::trace!("Boot info: {:x?}", flasher.boot_info());
    // use blflash::elf::CodeSegment;
    // let a = read("image/boot2image.bin")?;
    // let b = read("image/partition.bin")?;
    // let c = read("image/fwimage.bin")?;
    // let d = read("image/ro_params.dtb")?;
    // let segments = vec![
    //     CodeSegment::from_slice(0x0, &a),
    //     CodeSegment::from_slice(0xe000, &b),
    //     CodeSegment::from_slice(0xf000, &b),
    //     CodeSegment::from_slice(0x10000, &c),
    //     CodeSegment::from_slice(0x1f8000, &d),
    // ];
    // flasher.load_segments(segments.into_iter())?;
    let input_bytes = read(&opt.image)?;
    flasher.load_elf_to_flash(&input_bytes)?;

    flasher.reset()?;
    log::info!("Success");
    Ok(())
}

fn check(opt: CheckOpt) -> Result<(), Error> {
    let serial = serial::open(&opt.port)?;
    let mut flasher = Flasher::connect(serial, Some(BaudRate::Baud115200))?;

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
