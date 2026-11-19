//! tests/fixtures/의 Vagrantfile마다 k8slab 해석 결과가 tools/oracle.rb(실제 Ruby)의 결과와 같은지 확인한다.

use serde_json::Value;
use std::path::{Path, PathBuf};

fn vagrantfiles(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            vagrantfiles(&p, out);
        } else if p.file_name().is_some_and(|n| n == "Vagrantfile") {
            out.push(p);
        }
    }
}

/// 처음 다른 곳의 경로를 찾는다
fn first_diff(path: &str, a: &Value, b: &Value) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys()) {
                let p = format!("{path}.{k}");
                match (x.get(k), y.get(k)) {
                    (Some(xa), Some(yb)) => {
                        if let Some(d) = first_diff(&p, xa, yb) {
                            return Some(d);
                        }
                    }
                    (xa, yb) => return Some(format!("{p}: k8slab={xa:?} oracle={yb:?}")),
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Some(format!(
                    "{path}: 길이 k8slab={} oracle={}",
                    x.len(),
                    y.len()
                ));
            }
            x.iter()
                .zip(y)
                .enumerate()
                .find_map(|(i, (xa, yb))| first_diff(&format!("{path}[{i}]"), xa, yb))
        }
        _ if a == b => None,
        _ => Some(format!("{path}: k8slab={a} oracle={b}")),
    }
}

#[test]
fn matches_ruby_oracle() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut files = Vec::new();
    vagrantfiles(&root, &mut files);
    files.sort();
    assert!(!files.is_empty(), "tests/fixtures에 Vagrantfile이 없다");
    let mut failures = Vec::new();
    let mut checked = 0;
    for f in &files {
        let src = std::fs::read_to_string(f).unwrap();
        for arch in ["arm64", "x86_64"] {
            let expected_path = f.with_file_name(format!("plan.{arch}.json"));
            let expected: Value =
                serde_json::from_str(&std::fs::read_to_string(&expected_path).unwrap()).unwrap();
            let rel = f.strip_prefix(&root).unwrap().display().to_string();
            checked += 1;
            match k8slab::vagrantfile::plan(&src, arch) {
                Ok(got) => {
                    if let Some(d) = first_diff("", &got, &expected) {
                        failures.push(format!("[{arch}] {rel}: {d}"));
                    }
                }
                Err(e) => failures.push(format!("[{arch}] {rel}: {e}")),
            }
        }
    }
    if !failures.is_empty() {
        panic!(
            "{}/{checked}건이 다르다\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
    eprintln!("{checked}건 모두 같다");
}
