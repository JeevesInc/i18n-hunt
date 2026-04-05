//! Command-line argument parsing for `i18n-hunt`.

use clap::Parser;
use std::path::PathBuf;

use crate::core::Config;

/// CLI arguments accepted by the binary.
#[derive(Parser)]
#[command(name = "i18n-hunt")]
#[command(about = "Detect unused i18n keys using AST analysis")]
pub struct Args {
    /// Directory containing locale JSON files.
    #[arg(long)]
    locales: PathBuf,

    /// Source directory to scan for translation key usages.
    #[arg(long)]
    src: PathBuf,
}

impl Args {
    /// Converts CLI arguments into core analysis configuration.
    ///
    /// # Returns
    ///
    /// A [`Config`] ready for [`crate::core::run`].
    pub fn into_config(self) -> Config {
        Config {
            locales: self.locales,
            src: self.src,
        }
    }
}

/// Parses process arguments into [`Args`].
///
/// # Returns
///
/// Parsed CLI argument values.
///
/// # Panics
///
/// This function exits the process when parsing fails, as defined by `clap`.
pub fn parse() -> Args {
    Args::parse()
}
