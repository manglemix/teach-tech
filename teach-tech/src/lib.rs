#![feature(try_blocks)]
use std::{path::PathBuf, process::ExitCode};

use build::build_at_path;
use clap::{builder::OsStr, Parser, Subcommand};

pub mod build;

#[derive(Subcommand)]
pub enum Command {
    Build {
        #[arg(default_value = OsStr::from("."))]
        path: PathBuf,
    },
}

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

pub fn main() -> anyhow::Result<ExitCode> {
    let Cli { command } = Cli::parse();
    tracing_subscriber::fmt().init();
    match command {
        Command::Build { path } => build_at_path(&path),
    }
}
