//! Process-level NDJSON checks against the `lab-inspect` binary.
//!
//! Miri cannot spawn processes (`posix_spawn` / Windows `CreateProcess`).
//! This suite stays in ordinary `cargo test` and AddressSanitizer.

#![cfg(not(miri))]

use std::io::Write;
use std::process::{Command, Stdio};

use axiom_rs::foundation::Value;
use axiom_stdio::{decode_value, encode_value};

fn run_lines(lines: &[&str]) -> (Vec<Value>, String, i32) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lab-inspect"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("lab-inspect binary");
    {
        let mut stdin = child.stdin.take().unwrap();
        for line in lines {
            writeln!(stdin, "{line}").unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let values = stdout
        .lines()
        .map(|line| decode_value(line).unwrap())
        .collect();
    (
        values,
        String::from_utf8(output.stderr).unwrap(),
        output.status.code().unwrap_or(-1),
    )
}

fn path_display(value: &Value) -> &str {
    value
        .as_object()
        .unwrap()
        .get("error")
        .unwrap()
        .as_object()
        .unwrap()
        .get("path")
        .unwrap()
        .as_object()
        .unwrap()
        .get("display")
        .unwrap()
        .as_str()
        .unwrap()
}

fn success_volume(value: &Value) -> f64 {
    value
        .as_object()
        .unwrap()
        .get("value")
        .unwrap()
        .as_object()
        .unwrap()
        .get("volume")
        .unwrap()
        .as_float()
        .unwrap()
}

fn error_kind(value: &Value) -> &str {
    value
        .as_object()
        .unwrap()
        .get("error")
        .unwrap()
        .as_object()
        .unwrap()
        .get("kind")
        .unwrap()
        .as_str()
        .unwrap()
}

#[test]
fn process_discover_error_fix_malformed_and_eof() {
    let list = r#"{"v":{"$i":"1"},"cmd":"list"}"#;
    let get = r#"{"v":{"$i":"1"},"cmd":"get","name":"geom.axis_aligned_box"}"#;
    let bad = r#"{"v":{"$i":"1"},"cmd":"invoke","name":"geom.axis_aligned_box","input":{"size":{"x":"wide","y":{"$f":"2.0"},"z":{"$f":"3.0"}},"unit":"mm"},"correlation_id":"req-1"}"#;
    let ok = r#"{"v":{"$i":"1"},"cmd":"invoke","name":"geom.axis_aligned_box","input":{"size":{"x":{"$f":"2.0"},"y":{"$f":"3.0"},"z":{"$f":"4.0"}},"unit":"mm"},"correlation_id":"req-1"}"#;
    let (values, stderr, code) = run_lines(&[list, get, bad, ok, "{", list]);
    assert_eq!(code, 0);
    assert_eq!(values.len(), 6);
    assert_eq!(
        values[0].as_object().unwrap().get("ok"),
        Some(&Value::bool(true))
    );
    let discovered = values[1].as_object().unwrap().get("value").unwrap();
    assert!(encode_value(discovered).unwrap().contains("size"));
    assert_eq!(error_kind(&values[2]), "type_mismatch");
    assert_eq!(path_display(&values[2]), "size.x");
    assert!((success_volume(&values[3]) - 24.0).abs() < f64::EPSILON);
    assert_eq!(error_kind(&values[4]), "malformed_json");
    assert_eq!(
        values[5].as_object().unwrap().get("ok"),
        Some(&Value::bool(true))
    );
    assert!(stderr.contains("axiom-stdio"));
    assert!(
        !values
            .iter()
            .any(|value| encode_value(value).unwrap().contains("axiom-stdio"))
    );
}
