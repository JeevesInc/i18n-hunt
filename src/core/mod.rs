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

pub fn print_report(result: &AnalysisResult) {
    if result.unused_keys.is_empty() {
        println!("No unused translation keys found.");
        return;
    }

    println!("Unused translation keys:\n");

    for item in &result.unused_keys {
        println!(
            "[{}] {} -> {}",
            item.namespace,
            item.path.display(),
            item.key
        );
    }

    println!("\nTotal unused keys: {}", result.unused_keys.len());
}
