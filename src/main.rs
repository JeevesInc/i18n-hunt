use crate::core::error::I18nError;

mod cli;
mod core;

fn main() {
    // TODO: handle file case
    // TODO: better report
    // TODO: fail on unused
    // TODO: improve dynamic
    // TODO: consider root (by file - default or paramenter) to get the namespace
    // TODO: improve code docs
    // TODO: improve CLI docs

    if let Err(err) = run() {
        eprintln!("❌ Error: {}", err);
        std::process::exit(1);
    }
}

fn run() -> Result<(), I18nError> {
    let args = cli::parse();
    let config = args.into_config();

    let result = core::run(&config)?;

    core::print_report(&result);
    Ok(())
}
