use std::{collections::HashSet, fs::read_to_string};

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

fn main() {
    // TODO: handle multiple locale files
    // TODO: evaluate and handle unwraps

    let content = read_to_string("./fixtures/locales/Auth/Login.json").unwrap();
    let deserialized: Value = serde_json::from_str(&content).unwrap();

    let mut buf = String::new();
    let mut out = HashSet::new();

    flatten_into(&deserialized, &mut buf, &mut out);

    println!("{:?}", out);
}
