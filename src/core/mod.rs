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
    /// Glob patterns (relative to `src`) to skip source files/directories.
    pub src_exclude: Vec<String>,
    /// Glob patterns (relative to `locales`) to skip locale files/directories.
    pub locales_exclude: Vec<String>,
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
    let locales = locale::load_locales(&config.locales, &config.locales_exclude)?;
    let usages = source::collect_usages(&config.src, &config.src_exclude)?;
    Ok(analysis::analyze(&locales, &usages))
}

/// Prints a human-readable report of analysis findings.
///
/// Prints a success message when no unused keys or dynamic usages are found;
/// otherwise prints unused keys and unresolved dynamic usage sites.
///
/// # Arguments
///
/// * `result` - Analysis output to render to stdout.
pub fn print_report(result: &AnalysisResult) {
    if result.unused_keys.is_empty() && result.dynamic_usages.is_empty() {
        println!("No unused translation keys found.");
        return;
    }

    if !result.unused_keys.is_empty() {
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
    } else {
        println!("No unused translation keys found.");
    }

    if !result.dynamic_usages.is_empty() {
        println!("\nDynamic translation usage sites:\n");

        for item in &result.dynamic_usages {
            if item.namespaces.is_empty() {
                println!("{}:{} -> (no namespace)", item.path.display(), item.line);
            } else {
                println!(
                    "{}:{} -> [{}]",
                    item.path.display(),
                    item.line,
                    item.namespaces.join(", ")
                );
            }
        }

        println!("\nTotal dynamic usages: {}", result.dynamic_usages.len());
    }
}
