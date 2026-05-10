//! `i18n-hunt` binary entrypoint.
//!
//! Parses CLI arguments, executes analysis, and prints a human-readable report.

use crate::core::error::I18nError;

mod cli;
mod core;

/// Runs the CLI program and exits with a non-zero status on failure.
fn main() {
    if let Err(err) = run() {
        eprintln!("❌ Error: {}", err);
        std::process::exit(1);
    }
}

/// Executes one end-to-end analysis run.
///
/// # Returns
///
/// `Ok(())` when analysis and report rendering complete successfully.
///
/// # Errors
///
/// Returns [`I18nError`] when locale loading or source analysis fails.
fn run() -> Result<(), I18nError> {
    let args = cli::parse();
    let fix_mode = args.fix_mode();
    let log_mode = args.log_mode();
    let config = args.into_config()?;

    let result = core::run(&config)?;

    if fix_mode {
        let fix_result = core::fix::apply(&result.unused_keys)?;
        println!(
            "Auto-fix removed {} keys across {} files.",
            fix_result.removed_keys, fix_result.touched_files
        );
        return Ok(());
    }

    core::print_report(&result, log_mode);
    Ok(())
}
