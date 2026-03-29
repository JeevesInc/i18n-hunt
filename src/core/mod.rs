use oxc_allocator::Allocator;
use oxc_ast::ast::SourceType;
use oxc_ast_visit::Visit;
use oxc_parser::Parser as OxcParser;
use std::{fs::read_to_string, path::PathBuf};
use walkdir::WalkDir;

use crate::{CallCollector, Usage, core::locale::LocaleFile};

pub mod locale;

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
    let usages = collect_usages(&config.src)?;
    Ok(AnalysisResult { locales, usages })
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
