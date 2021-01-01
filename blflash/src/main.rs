use blflash::{check, dump, flash, Opt};
use env_logger::Env;
use main_error::MainError;

#[paw::main]
fn main(args: Opt) -> Result<(), MainError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("blflash=trace"))
        .format_timestamp(None)
        .init();

    match args {
        Opt::Flash(opt) => flash(opt)?,
        Opt::Check(opt) => check(opt)?,
        Opt::Dump(opt) => dump(opt)?,
    };

    Ok(())
}
