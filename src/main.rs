mod browser;
mod chart;
mod worker;

use clap::Parser;
use color_eyre::eyre::{Result, WrapErr};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about = "Explore Cargo build directories and remove selected outputs"
)]
struct Args {
    /// Directory to scan recursively
    #[arg(default_value = ".")]
    directory: PathBuf,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    let root = args
        .directory
        .canonicalize()
        .wrap_err_with(|| format!("Cannot open {:?}", args.directory))?;
    spdlog::info!("Scanning {:?}", root);
    browser::run(root)
}
