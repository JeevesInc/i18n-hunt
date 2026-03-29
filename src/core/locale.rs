use serde_json::Value;
use std::{collections::HashSet, fs::read_to_string, path::PathBuf};
use walkdir::WalkDir;

pub struct LocaleFile {
    pub namespace: String,
    pub path: PathBuf,
    pub keys: HashSet<String>,
}

// TODO: handle single file?
pub fn load_locales(locales_dir: &PathBuf) -> Result<Vec<LocaleFile>, &str> {
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
