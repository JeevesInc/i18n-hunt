use std::path::PathBuf;

use crate::core::{analysis::AnalysisResult, error::I18nError};

pub mod analysis;
pub mod error;
pub mod locale;
pub mod source;

pub struct Config {
    pub locales: PathBuf,
    pub src: PathBuf,
}

pub fn run(config: &Config) -> Result<AnalysisResult, I18nError> {
    let locales = locale::load_locales(&config.locales)?;
    let usages = source::collect_usages(&config.src)?;
    Ok(analysis::analyze(&locales, &usages))
}
