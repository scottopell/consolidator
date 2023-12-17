use clap::Parser;
use consolidator::processor;
use thiserror::Error;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Error, Debug)]
pub enum Error {
    #[error("Processing Error: {0}")]
    Processing(#[from] processor::Error),
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Consolidator {
    target_path: std::path::PathBuf,
}

fn main() -> Result<(), Error> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(env_filter)
        .init();

    info!("Hello, world!");

    let args = Consolidator::parse();

    processor::process(&args.target_path)?;

    Ok(())
}
