use std::path::PathBuf;

use crate::{Usage, core::locale::LocaleFile};

pub mod locale;
pub mod source;

pub struct Config {
    pub locales: PathBuf,
    pub src: PathBuf,
}

pub struct AnalysisResult {
    pub locales: Vec<LocaleFile>,
    pub usages: Vec<Usage>,
}

pub fn run(config: &Config) -> Result<AnalysisResult, &str> {
    let locales = locale::load_locales(&config.locales)?;
    let usages = source::collect_usages(&config.src)?;
    Ok(AnalysisResult { locales, usages })
}
