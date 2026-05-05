//! Command-line argument parsing for `i18n-hunt`.

use std::{fs::read_to_string, path::Path};

use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;

use crate::core::{Config, error::I18nError};

/// CLI arguments accepted by the binary.
#[derive(Parser)]
#[command(name = "i18n-hunt")]
#[command(about = "Detect unused i18n keys using AST analysis")]
pub struct Args {
    /// Locale directory (or a specific locale JSON file).
    #[arg(long)]
    locales: Option<PathBuf>,

    /// Source directory or a specific source file to scan.
    #[arg(long)]
    src: Option<PathBuf>,

    /// Optional config file path (TOML). If omitted, `i18n-hunt.toml` is
    /// loaded automatically when present.
    #[arg(long)]
    config: Option<PathBuf>,
}

impl Args {
    /// Converts CLI arguments into core analysis configuration.
    ///
    /// # Returns
    ///
    /// A [`Config`] ready for [`crate::core::run`].
    pub fn into_config(self) -> Result<Config, I18nError> {
        let file_config = load_file_config(self.config.as_deref())?;

        let locales = self.locales.or(file_config.locales).ok_or_else(|| {
            I18nError::Config("missing locales path (use --locales or i18n-hunt.toml)".to_string())
        })?;
        if !(locales.is_dir() || locales.is_file()) {
            return Err(I18nError::Config(format!(
                "locales path must be a directory or file: '{}'",
                locales.display()
            )));
        }

        let src = self.src.or(file_config.src).ok_or_else(|| {
            I18nError::Config("missing src path (use --src or i18n-hunt.toml)".to_string())
        })?;
        if !(src.is_dir() || src.is_file()) {
            return Err(I18nError::Config(format!(
                "src path must be a directory or file: '{}'",
                src.display()
            )));
        }

        Ok(Config {
            locales,
            src,
            src_exclude: file_config.src_exclude.unwrap_or_default(),
            locales_exclude: file_config.locales_exclude.unwrap_or_default(),
        })
    }
}

#[derive(Default, Deserialize)]
struct FileConfig {
    locales: Option<PathBuf>,
    src: Option<PathBuf>,
    src_exclude: Option<Vec<String>>,
    locales_exclude: Option<Vec<String>>,
}

fn load_file_config(config_arg: Option<&Path>) -> Result<FileConfig, I18nError> {
    let explicit_path = config_arg.map(Path::to_path_buf);
    let default_path = PathBuf::from("i18n-hunt.toml");

    let path = match explicit_path {
        Some(path) => path,
        None if default_path.exists() => default_path,
        None => return Ok(FileConfig::default()),
    };

    let raw = read_to_string(&path)?;
    let parsed = toml::from_str::<FileConfig>(&raw).map_err(|err| {
        I18nError::Config(format!("failed to parse '{}': {}", path.display(), err))
    })?;

    Ok(parsed)
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

#[cfg(test)]
mod tests {
    use std::{
        fs::{create_dir_all, write},
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::Args;
    use crate::core::error::I18nError;

    fn make_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("i18n-hunt-tests-{}-{}", std::process::id(), nanos));
        create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    #[test]
    fn into_config_reads_explicit_config_and_applies_excludes() {
        let dir = make_temp_dir();
        let locales = dir.join("locales");
        let src = dir.join("src");
        create_dir_all(&locales).expect("locales dir");
        create_dir_all(&src).expect("src dir");

        let config_path = dir.join("i18n-hunt.toml");
        write(
            &config_path,
            format!(
                "locales = \"{}\"\nsrc = \"{}\"\nsrc_exclude = [\"**/*.spec.ts\"]\nlocales_exclude = [\"Legacy/**\"]\n",
                locales.display(),
                src.display()
            ),
        )
        .expect("config file write");

        let args = Args {
            locales: None,
            src: None,
            config: Some(config_path),
        };

        let config = args.into_config().expect("config should parse");
        assert_eq!(config.locales, locales);
        assert_eq!(config.src, src);
        assert_eq!(config.src_exclude, vec!["**/*.spec.ts"]);
        assert_eq!(config.locales_exclude, vec!["Legacy/**"]);
    }

    #[test]
    fn into_config_returns_error_for_missing_inputs() {
        let args = Args {
            locales: None,
            src: None,
            config: None,
        };

        match args.into_config() {
            Err(I18nError::Config(message)) => assert!(message.contains("missing locales path")),
            Ok(_) => panic!("expected config error but got config"),
            Err(other) => panic!("expected config error, got: {other}"),
        }
    }
}
