//! End-to-end native, command, and stdio checks for the reference host.

use std::io::Cursor;

use axiom_rs::command::{decode, execute};
use axiom_rs::foundation::{ErrorKind, Value};
use axiom_rs::runtime::Runtime;
use axiom_stdio::{decode_value, encode_value, serve};
use lab_inspect::register;

fn runtime() -> Runtime {
    let runtime = Runtime::new();
    register(&runtime).unwrap();
    runtime
}

fn obj(fields: &[(&str, Value)]) -> Value {
    Value::try_object(fields.iter().cloned()).unwrap()
}

fn float(value: f64) -> Value {
    Value::try_float(value).unwrap()
}

fn cmd(fields: &[(&str, Value)]) -> axiom_rs::Command {
    decode(&obj(fields)).unwrap()
}

fn box_input(x: Value, y: Value, z: Value) -> Value {
    obj(&[
        ("size", obj(&[("x", x), ("y", y), ("z", z)])),
        ("unit", Value::string("mm")),
    ])
}

#[test]
fn discovery_then_nested_error_then_success() {
    let runtime = runtime();
    let listed = execute(
        &runtime,
        &cmd(&[("v", Value::integer(1)), ("cmd", Value::string("list"))]),
    );
    let names: Vec<_> = listed
        .value()
        .unwrap()
        .as_list()
        .unwrap()
        .iter()
        .map(|item| {
            item.as_object()
                .unwrap()
                .get("name")
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert!(names.contains(&"geom.axis_aligned_box".to_owned()));
    let discovered = execute(
        &runtime,
        &cmd(&[
            ("v", Value::integer(1)),
            ("cmd", Value::string("get")),
            ("name", Value::string("geom.axis_aligned_box")),
        ]),
    );
    let input_contract = discovered
        .value()
        .unwrap()
        .as_object()
        .unwrap()
        .get("input")
        .unwrap();
    assert!(encode_value(input_contract).unwrap().contains("size"));
    let bad = execute(
        &runtime,
        &cmd(&[
            ("v", Value::integer(1)),
            ("cmd", Value::string("invoke")),
            ("name", Value::string("geom.axis_aligned_box")),
            (
                "input",
                box_input(Value::from("wide"), float(2.0), float(3.0)),
            ),
            ("correlation_id", Value::string("req-1")),
        ]),
    );
    assert_eq!(bad.error().unwrap().kind(), ErrorKind::TypeMismatch);
    assert_eq!(bad.error().unwrap().path().unwrap().to_string(), "size.x");
    let ok = execute(
        &runtime,
        &cmd(&[
            ("v", Value::integer(1)),
            ("cmd", Value::string("invoke")),
            ("name", Value::string("geom.axis_aligned_box")),
            ("input", box_input(float(2.0), float(3.0), float(4.0))),
            ("correlation_id", Value::string("req-1")),
        ]),
    );
    let volume = ok
        .value()
        .unwrap()
        .as_object()
        .unwrap()
        .get("volume")
        .unwrap()
        .as_float()
        .unwrap();
    assert!((volume - 24.0).abs() < f64::EPSILON);
    assert_eq!(ok.correlation_id(), Some("req-1"));
}

#[test]
fn stdio_round_trip_matches_command() {
    let runtime = runtime();
    let request = concat!(
        r#"{"v":{"$i":"1"},"cmd":"invoke","name":"stats.summarize","input":{"samples":[{"$i":"2"},{"$i":"4"}]},"correlation_id":"c1"}"#,
        "\n"
    );
    let mut output = Vec::new();
    let mut diag = Vec::new();
    serve(
        &runtime,
        Cursor::new(request.as_bytes()),
        &mut output,
        &mut diag,
    )
    .unwrap();
    let json = String::from_utf8(output).unwrap();
    let value = decode_value(json.trim()).unwrap();
    let mean = value
        .as_object()
        .unwrap()
        .get("value")
        .unwrap()
        .as_object()
        .unwrap()
        .get("mean")
        .unwrap()
        .as_float()
        .unwrap();
    assert!((mean - 3.0).abs() < f64::EPSILON);
    assert!(diag.is_empty());
}

#[test]
fn dilute_business_failure_and_temp_success() {
    let runtime = runtime();
    let fail = execute(
        &runtime,
        &decode(&obj(&[
            ("v", Value::integer(1)),
            ("cmd", Value::string("invoke")),
            ("name", Value::string("chem.dilute")),
            (
                "input",
                obj(&[
                    ("stock_mM", float(1.0)),
                    ("target_mM", float(2.0)),
                    ("volume_mL", float(10.0)),
                ]),
            ),
            ("correlation_id", Value::string("chem")),
        ]))
        .unwrap(),
    );
    assert_eq!(fail.error().unwrap().kind(), ErrorKind::BusinessFailure);

    let ok = execute(
        &runtime,
        &decode(&obj(&[
            ("v", Value::integer(1)),
            ("cmd", Value::string("invoke")),
            ("name", Value::string("temp.convert")),
            (
                "input",
                obj(&[
                    ("value", float(0.0)),
                    ("from", Value::string("C")),
                    ("to", Value::string("K")),
                ]),
            ),
            ("correlation_id", Value::string("temp")),
        ]))
        .unwrap(),
    );
    let kelvin = ok
        .value()
        .unwrap()
        .as_object()
        .unwrap()
        .get("value")
        .unwrap()
        .as_float()
        .unwrap();
    assert!((kelvin - 273.15).abs() < 1e-9);
}
