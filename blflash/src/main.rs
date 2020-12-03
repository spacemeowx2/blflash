use std::fs::read;

use blflash::{Config, Flasher};
use main_error::MainError;
use pico_args::Arguments;
use serial::BaudRate;
use env_logger::Env;

fn help() -> Result<(), MainError> {
    println!("Usage: espflash [--board-info] [--ram] <serial> <elf image>");
    Ok(())
}

fn main() -> Result<(), MainError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("blflash=trace")).init();
    let mut args = Arguments::from_env();
    let config = Config::load();

    log::trace!("trace on");

    if args.contains(["-h", "--help"]) {
        return help();
    }

    let _ram = args.contains("--ram");

    let mut serial: Option<String> = args.free_from_str()?;
    let mut elf: Option<String> = args.free_from_str()?;

    if elf.is_none() && config.connection.serial.is_some() {
        elf = serial.take();
        serial = config.connection.serial;
    }

    let input: String = match elf {
        Some(input) => input,
        _ => return help(),
    };
    let input_bytes = read(&input)?;

    let serial: String = match serial {
        Some(serial) => serial,
        _ => return help(),
    };

    let serial = serial::open(&serial)?;
    let mut flasher = Flasher::connect(serial, Some(BaudRate::BaudOther(500_000)))?;

    log::info!("Bootrom version: {}", flasher.boot_info().bootrom_version);
    flasher.load_elf_to_flash(&input_bytes)?;

    log::info!("Success");

    Ok(())
}
