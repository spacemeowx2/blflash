use std::path::PathBuf;
use std::process::{exit, Command, ExitStatus, Stdio};

use blflash::{
    chip::{Bl602, Chip},
    flash, Boot2Opt, Connection, FlashOpt,
};
use cargo_project::{Artifact, Profile, Project};
use color_eyre::{Report, Result};
use env_logger::Env;
use structopt::StructOpt;

#[derive(StructOpt)]
struct BlflashOpt {
    #[structopt(flatten)]
    conn: Connection,
    /// Don't skip if hash matches
    #[structopt(short, long)]
    force: bool,
    #[structopt(flatten)]
    boot: Boot2Opt,
    #[structopt(long)]
    release: bool,
    #[structopt(long)]
    example: Option<String>,
    #[structopt(long)]
    features: Option<String>,
}

#[derive(StructOpt)]
enum Opt {
    Blflash(BlflashOpt),
}

fn blflash_main(args: BlflashOpt) -> Result<()> {
    let chip = Bl602;
    let target = chip.target();

    let status = build(args.release, &args.example, &args.features, target);
    if !status.success() {
        exit_with_process_status(status)
    }

    let path = get_artifact_path(target, args.release, &args.example)
        .expect("Could not find the build artifact path");

    let flash_opt = FlashOpt {
        conn: args.conn,
        image: path,
        force: args.force,
        boot: args.boot,
    };

    flash(flash_opt)?;

    Ok(())
}

#[paw::main]
fn main(args: Opt) -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("blflash=trace"))
        .format_timestamp(None)
        .init();

    match args {
        Opt::Blflash(opt) => blflash_main(opt),
    }
}

fn get_artifact_path(target: &str, release: bool, example: &Option<String>) -> Result<PathBuf> {
    let project = Project::query(".").unwrap();

    let artifact = match example {
        Some(example) => Artifact::Example(example.as_str()),
        None => Artifact::Bin(project.name()),
    };

    let profile = if release {
        Profile::Release
    } else {
        Profile::Dev
    };

    let host = "x86_64-unknown-linux-gnu";
    project
        .path(artifact, profile, Some(target), host)
        .map_err(Report::msg)
}

fn build(
    release: bool,
    example: &Option<String>,
    features: &Option<String>,
    target: &str,
) -> ExitStatus {
    let mut args: Vec<String> = vec![];

    if release {
        args.push("--release".to_string());
    }

    match example {
        Some(example) => {
            args.push("--example".to_string());
            args.push(example.to_string());
        }
        None => {}
    }

    match features {
        Some(features) => {
            args.push("--features".to_string());
            args.push(features.to_string());
        }
        None => {}
    }

    let mut command = Command::new("cargo");

    args.push("--target".to_string());
    args.push(target.to_string());

    command
        .arg("build")
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
}

#[cfg(unix)]
fn exit_with_process_status(status: ExitStatus) -> ! {
    use std::os::unix::process::ExitStatusExt;
    let code = status.code().or_else(|| status.signal()).unwrap_or(1);

    exit(code)
}

#[cfg(not(unix))]
fn exit_with_process_status(status: ExitStatus) -> ! {
    let code = status.code().unwrap_or(1);

    exit(code)
}
