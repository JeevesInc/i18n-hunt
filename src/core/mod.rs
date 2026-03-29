use std::path::PathBuf;

use crate::core::analysis::AnalysisResult;

pub mod analysis;
pub mod locale;
pub mod source;

pub struct Config {
    pub locales: PathBuf,
    pub src: PathBuf,
}

pub fn run(config: &Config) -> Result<AnalysisResult, &str> {
    let locales = locale::load_locales(&config.locales)?;
    let usages = source::collect_usages(&config.src)?;
    Ok(analysis::analyze(&locales, &usages))
}
