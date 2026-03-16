use std::{collections::HashSet, fs::read_to_string, path::PathBuf};
use walkdir::WalkDir;

use serde_json::Value;

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

struct LocaleFile {
    namespace: String,
    path: PathBuf,
    keys: HashSet<String>,
}

fn main() {
    // TODO: based on user input or config file
    let locales_dir = "./fixtures/locales";
    let mut locales: Vec<LocaleFile> = vec![];

    // TODO: evaluate and handle unwraps
    for entry in WalkDir::new(locales_dir) {
        let entry = entry.unwrap();
        // TODO: add better check for json
        if entry.file_type().is_file() {
            let mut buf = String::new();
            let mut out = HashSet::new();

            let content = read_to_string(entry.path()).unwrap();
            let deserialized: Value = serde_json::from_str(&content).unwrap();
            flatten_into(&deserialized, &mut buf, &mut out);

            // TODO: refactor to  derive_namespace function?
            // TODO: hadle unwraps and possible errors
            let relative = entry.path().strip_prefix(locales_dir).unwrap();
            let without_ext = relative.with_extension("");
            let normalized = without_ext.to_string_lossy().replace('\\', "/");

            // TODO: should we do impl for new
            let locale_file = LocaleFile {
                namespace: normalized,
                path: entry.path().to_path_buf(),
                keys: out,
            };

            println!("File: {}", locale_file.path.display());
            println!("Namespace: {}", locale_file.namespace);
            println!("Keys: {:?}", locale_file.keys);
            println!();

            locales.push(locale_file);
        }
    }
}
