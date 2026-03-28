use oxc_allocator::Allocator;
use oxc_ast::ast::SourceType;
use oxc_ast_visit::Visit;
use oxc_parser::Parser as OxcParser;
use serde_json::Value;
use std::{collections::HashSet, fs::read_to_string, path::PathBuf};
use walkdir::WalkDir;

use crate::{CallCollector, LocaleFile, Usage};

pub struct Config {
    pub locales: PathBuf,
    pub src: PathBuf,
}

pub struct AnalysisResult {
    pub locales: Vec<LocaleFile>,
    pub usages: Vec<Usage>,
}

pub fn run(config: &Config) -> Result<AnalysisResult, &str> {
    let locales = load_locales(&config.locales)?;
    let usages = collect_usages(&config.src)?;
    Ok(AnalysisResult { locales, usages })
}

// TODO: handle single file?
fn load_locales(locales_dir: &PathBuf) -> Result<Vec<LocaleFile>, &str> {
    let mut locales: Vec<LocaleFile> = vec![];

    for entry in WalkDir::new(&locales_dir) {
        let entry = entry.unwrap();
        // TODO: add better check for json
        if entry.file_type().is_file() {
            let mut buf = String::new();
            let mut out = HashSet::new();

            let content = read_to_string(entry.path()).unwrap();
            let deserialized: Value = serde_json::from_str(&content).unwrap();
            flatten_into(&deserialized, &mut buf, &mut out);

            // TODO: refactor to derive_namespace function?
            // TODO: hadle unwraps and possible errors
            let relative = entry.path().strip_prefix(&locales_dir).unwrap();
            let without_ext = relative.with_extension("");
            let normalized = without_ext.to_string_lossy().replace('\\', "/");

            // TODO: should we do impl for new
            let locale_file = LocaleFile {
                namespace: normalized,
                path: entry.path().to_path_buf(),
                keys: out,
            };

            locales.push(locale_file);
        }
    }

    Ok(locales)
}

fn flatten_into(value: &Value, buf: &mut String, out: &mut HashSet<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let previus_state = buf.len();

                if !buf.is_empty() {
                    buf.push('.');
                }

                buf.push_str(&k);

                flatten_into(v, buf, out);

                buf.truncate(previus_state);
            }
        }
        Value::String(_) => {
            if !buf.is_empty() {
                out.insert(buf.clone());
            }
        }
        _ => {}
    }
}

fn collect_usages(source_dir: &PathBuf) -> Result<Vec<Usage>, &str> {
    let mut all_usages: Vec<Usage> = vec![];

    for entry in WalkDir::new(source_dir) {
        let entry = entry.unwrap();

        if entry.file_type().is_file() {
            let path = entry.path();

            let is_supported = matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("ts") | Some("tsx") | Some("js") | Some("jsx")
            );

            if !is_supported {
                continue;
            }

            let source_text = read_to_string(path).unwrap();

            let allocator = Allocator::default();
            let source_type = SourceType::from_path(path).unwrap();
            let parser = OxcParser::new(&allocator, &source_text, source_type);
            let ret = parser.parse();

            if !ret.errors.is_empty() {
                println!("Parse errors:");
                for err in ret.errors {
                    println!("{:?}", err);
                }
            }

            let mut collector = CallCollector {
                namespaces: Vec::new(),
                usages: Vec::new(),
            };

            collector.visit_program(&ret.program);

            all_usages.extend(collector.usages);
        }
    }

    Ok(all_usages)
}
