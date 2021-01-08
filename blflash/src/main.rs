use blflash::{check, dump, flash, Opt};
use env_logger::Env;
use futures::executor::block_on;
use main_error::MainError;

#[paw::main]
fn main(args: Opt) -> Result<(), MainError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("blflash=trace"))
        .format_timestamp(None)
        .init();

    block_on(async {
        match args {
            Opt::Flash(opt) => flash(opt).await,
            Opt::Check(opt) => check(opt).await,
            Opt::Dump(opt) => dump(opt).await,
        }
    })?;

    Ok(())
}
