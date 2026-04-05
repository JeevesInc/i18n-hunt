//! Core analysis pipeline for unused i18n translation keys.

use std::path::PathBuf;

use crate::core::{analysis::AnalysisResult, error::I18nError};

pub mod analysis;
pub mod error;
pub mod locale;
pub mod source;

/// Filesystem inputs required by the core analysis pipeline.
pub struct Config {
    /// Root directory that contains locale JSON files.
    pub locales: PathBuf,
    /// Root directory that contains source files to analyze.
    pub src: PathBuf,
}

/// Runs locale loading, source scanning, and unused-key analysis.
///
/// # Arguments
///
/// * `config` - Input directories for locales and source code.
///
/// # Returns
///
/// An [`AnalysisResult`] containing all detected unused keys.
///
/// # Errors
///
/// Returns [`I18nError`] if any I/O, parsing, or traversal step fails.
pub fn run(config: &Config) -> Result<AnalysisResult, I18nError> {
    let locales = locale::load_locales(&config.locales)?;
    let usages = source::collect_usages(&config.src)?;
    Ok(analysis::analyze(&locales, &usages))
}

/// Prints a human-readable report of analysis findings.
///
/// Prints a success message when no unused keys are found; otherwise prints one
/// line per unused key and a final total.
///
/// # Arguments
///
/// * `result` - Analysis output to render to stdout.
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
