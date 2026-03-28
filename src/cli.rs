use clap::Parser;
use std::path::PathBuf;

use crate::core::Config;

#[derive(Parser)]
#[command(name = "i18n-hunt")]
#[command(about = "Detect unused i18n keys using AST analysis")]
pub struct Args {
    #[arg(long)]
    locales: PathBuf,

    #[arg(long)]
    src: PathBuf,
}

impl Args {
    pub fn into_config(self) -> Config {
        Config {
            locales: self.locales,
            src: self.src,
        }
    }
}

pub fn parse() -> Args {
    Args::parse()
}
