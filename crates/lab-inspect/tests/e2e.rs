//! End-to-end native, command, and stdio checks for the reference host.

use std::io::Cursor;

use axiom_rs::command::{decode, execute};
use axiom_rs::foundation::{Error, ErrorKind, Path, PathSegment, Value};
use axiom_rs::runtime::Runtime;
use axiom_rs::{
    BusinessFailure, Capability, CapabilityCategory, CapabilityDescriptor, CapabilityName,
    ExecutionContext, FieldContract, TypeContract,
};
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

fn invoke_envelope(name: &str, input: Value, correlation: &str) -> Value {
    invoke_envelope_with_parent(name, input, correlation, None)
}

fn invoke_envelope_with_parent(
    name: &str,
    input: Value,
    correlation: &str,
    parent: Option<&str>,
) -> Value {
    let mut fields = vec![
        ("v", Value::integer(1)),
        ("cmd", Value::string("invoke")),
        ("name", Value::string(name)),
        ("input", input),
        ("correlation_id", Value::string(correlation)),
    ];
    if let Some(parent) = parent {
        fields.push(("parent_id", Value::string(parent)));
    }
    obj(&fields)
}

fn via_command(runtime: &Runtime, request: &Value) -> Value {
    execute(runtime, &decode(request).unwrap()).to_value()
}

fn via_stdio(runtime: &Runtime, request: &Value) -> Value {
    let mut line = encode_value(request).unwrap();
    line.push('\n');
    let mut output = Vec::new();
    let mut diag = Vec::new();
    serve(
        runtime,
        Cursor::new(line.into_bytes()),
        &mut output,
        &mut diag,
    )
    .unwrap();
    decode_value(String::from_utf8(output).unwrap().trim()).unwrap()
}

fn via_runtime(runtime: &Runtime, request: &Value) -> Value {
    let command = decode(request).unwrap();
    match command {
        axiom_rs::Command::List => ok_envelope(Value::list(
            runtime
                .list()
                .iter()
                .map(axiom_rs::CapabilityDescriptor::to_value),
        )),
        axiom_rs::Command::Get { name } => ok_envelope(
            runtime
                .get(&name)
                .map(|descriptor| descriptor.to_value())
                .unwrap_or(Value::null()),
        ),
        axiom_rs::Command::Invoke {
            name,
            input,
            context,
        } => envelope_from_invoke(runtime.invoke(&name, input, &context), &context),
    }
}

fn ok_envelope(value: Value) -> Value {
    obj(&[
        ("v", Value::integer(1)),
        ("ok", Value::bool(true)),
        ("value", value),
    ])
}

fn envelope_from_invoke(result: Result<Value, Error>, context: &ExecutionContext) -> Value {
    let command_like = match result {
        Ok(value) => obj(&[
            ("v", Value::integer(1)),
            ("ok", Value::bool(true)),
            ("value", value),
            (
                "correlation_id",
                Value::string(context.correlation_id().as_str()),
            ),
        ]),
        Err(error) => failure_envelope(&error, context),
    };
    if let Some(parent) = context.parent_id() {
        let mut fields: Vec<(String, Value)> = command_like
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.to_owned(), v.clone()))
            .collect();
        fields.push(("parent_id".into(), Value::string(parent.as_str())));
        return Value::try_object(fields).unwrap();
    }
    command_like
}

fn failure_envelope(error: &Error, context: &ExecutionContext) -> Value {
    obj(&[
        ("v", Value::integer(1)),
        ("ok", Value::bool(false)),
        (
            "error",
            obj(&[
                ("kind", Value::string(error.kind().as_str())),
                ("message", Value::string(error.message())),
                ("path", path_value(error.path())),
                ("details", error.details().cloned().unwrap_or(Value::null())),
            ]),
        ),
        (
            "correlation_id",
            Value::string(context.correlation_id().as_str()),
        ),
    ])
}

fn path_value(path: Option<&Path>) -> Value {
    match path {
        None => Value::null(),
        Some(path) => obj(&[
            (
                "segments",
                Value::list(path.segments().iter().map(segment_value)),
            ),
            ("display", Value::string(path.to_string())),
        ]),
    }
}

fn segment_value(segment: &PathSegment) -> Value {
    match segment {
        PathSegment::Field(name) => obj(&[
            ("kind", Value::string("field")),
            ("name", Value::string(name.clone())),
        ]),
        PathSegment::Index(index) => obj(&[
            ("kind", Value::string("index")),
            ("index", Value::integer(i64::try_from(*index).unwrap())),
        ]),
    }
}

fn assert_same_envelope(runtime: &Runtime, request: &Value) {
    let command = via_command(runtime, request);
    let stdio = via_stdio(runtime, request);
    let native = via_runtime(runtime, request);
    assert_eq!(command, stdio);
    assert_eq!(command, native);
}

fn box_input(x: Value, y: Value, z: Value) -> Value {
    obj(&[
        ("size", obj(&[("x", x), ("y", y), ("z", z)])),
        ("unit", Value::string("mm")),
    ])
}

#[test]
fn fixtures_agree_across_runtime_command_and_stdio() {
    let runtime = runtime();
    assert_same_envelope(
        &runtime,
        &obj(&[("v", Value::integer(1)), ("cmd", Value::string("list"))]),
    );
    assert_same_envelope(
        &runtime,
        &obj(&[
            ("v", Value::integer(1)),
            ("cmd", Value::string("get")),
            ("name", Value::string("geom.axis_aligned_box")),
        ]),
    );
    assert_same_envelope(
        &runtime,
        &invoke_envelope(
            "geom.axis_aligned_box",
            box_input(Value::from("wide"), float(2.0), float(3.0)),
            "bad-nested",
        ),
    );
    assert_same_envelope(
        &runtime,
        &invoke_envelope(
            "chem.dilute",
            obj(&[
                ("stock_mM", float(1.0)),
                ("target_mM", float(2.0)),
                ("volume_mL", float(10.0)),
            ]),
            "chem",
        ),
    );
    assert_same_envelope(
        &runtime,
        &invoke_envelope("missing.capability", Value::null(), "unknown"),
    );
    assert_same_envelope(
        &runtime,
        &invoke_envelope(
            "stats.summarize",
            obj(&[(
                "samples",
                Value::list([Value::integer(2), Value::integer(4)]),
            )]),
            "ok-stats",
        ),
    );
}

struct EchoContext;

impl Capability for EchoContext {
    fn invoke(&self, input: Value, context: &ExecutionContext) -> Result<Value, BusinessFailure> {
        let parent = context.parent_id().map(|id| id.as_str()).unwrap_or("");
        let seen = obj(&[
            (
                "correlation_id",
                Value::string(context.correlation_id().as_str()),
            ),
            ("parent_id", Value::string(parent)),
        ]);
        let fail = input
            .as_object()
            .and_then(|object| object.get("fail"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if fail {
            Err(BusinessFailure::new("echo requested failure").with_details(seen))
        } else {
            Ok(seen)
        }
    }
}

fn register_echo(runtime: &Runtime) {
    let input =
        TypeContract::object(vec![FieldContract::new("fail", TypeContract::Bool, true)]).unwrap();
    let output = TypeContract::object(vec![
        FieldContract::new("correlation_id", TypeContract::String, true),
        FieldContract::new("parent_id", TypeContract::String, true),
    ])
    .unwrap();
    runtime
        .register(
            CapabilityDescriptor::new(
                CapabilityName::parse("test.echo_context").unwrap(),
                "echo execution context",
                CapabilityCategory::parse("lab").unwrap(),
                input,
                output,
            )
            .unwrap(),
            EchoContext,
        )
        .unwrap();
}

fn host_seen_ids(envelope: &Value) -> (&str, &str) {
    let object = envelope.as_object().unwrap();
    if object.get("ok").and_then(Value::as_bool) == Some(true) {
        let value = object.get("value").unwrap().as_object().unwrap();
        (
            value.get("correlation_id").unwrap().as_str().unwrap(),
            value.get("parent_id").unwrap().as_str().unwrap(),
        )
    } else {
        let details = object
            .get("error")
            .unwrap()
            .as_object()
            .unwrap()
            .get("details")
            .unwrap()
            .as_object()
            .unwrap();
        (
            details.get("correlation_id").unwrap().as_str().unwrap(),
            details.get("parent_id").unwrap().as_str().unwrap(),
        )
    }
}

#[test]
fn child_context_is_observed_by_the_host_across_entries() {
    let runtime = runtime();
    register_echo(&runtime);
    let ok = invoke_envelope_with_parent(
        "test.echo_context",
        obj(&[("fail", Value::bool(false))]),
        "child-ok",
        Some("root-ok"),
    );
    assert_same_envelope(&runtime, &ok);
    assert_eq!(
        host_seen_ids(&via_command(&runtime, &ok)),
        ("child-ok", "root-ok")
    );
    let fail = invoke_envelope_with_parent(
        "test.echo_context",
        obj(&[("fail", Value::bool(true))]),
        "child-err",
        Some("root-err"),
    );
    assert_same_envelope(&runtime, &fail);
    assert_eq!(
        host_seen_ids(&via_command(&runtime, &fail)),
        ("child-err", "root-err")
    );
    assert_eq!(
        via_command(&runtime, &fail)
            .as_object()
            .unwrap()
            .get("parent_id")
            .and_then(Value::as_str),
        Some("root-err")
    );
}

struct BadOutput;

impl Capability for BadOutput {
    fn invoke(&self, _input: Value, _context: &ExecutionContext) -> Result<Value, BusinessFailure> {
        Ok(Value::integer(1))
    }
}

#[test]
fn output_violation_agrees_across_entries() {
    let runtime = runtime();
    runtime
        .register(
            CapabilityDescriptor::new(
                CapabilityName::parse("test.bad_output").unwrap(),
                "returns the wrong type",
                CapabilityCategory::parse("lab").unwrap(),
                TypeContract::Null,
                TypeContract::String,
            )
            .unwrap(),
            BadOutput,
        )
        .unwrap();
    assert_same_envelope(
        &runtime,
        &invoke_envelope("test.bad_output", Value::null(), "out"),
    );
    let encoded = via_command(
        &runtime,
        &invoke_envelope("test.bad_output", Value::null(), "out"),
    );
    let kind = encoded
        .as_object()
        .unwrap()
        .get("error")
        .unwrap()
        .as_object()
        .unwrap()
        .get("kind")
        .unwrap()
        .as_str()
        .unwrap();
    assert_eq!(kind, ErrorKind::OutputContractViolation.as_str());
}

fn listed_names(runtime: &Runtime) -> Vec<String> {
    execute(
        runtime,
        &decode(&obj(&[
            ("v", Value::integer(1)),
            ("cmd", Value::string("list")),
        ]))
        .unwrap(),
    )
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
    .collect()
}

#[test]
fn discovery_then_nested_error_then_success() {
    let runtime = runtime();
    assert!(listed_names(&runtime).contains(&"geom.axis_aligned_box".to_owned()));
    let discovered = execute(
        &runtime,
        &decode(&obj(&[
            ("v", Value::integer(1)),
            ("cmd", Value::string("get")),
            ("name", Value::string("geom.axis_aligned_box")),
        ]))
        .unwrap(),
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
        &decode(&invoke_envelope(
            "geom.axis_aligned_box",
            box_input(Value::from("wide"), float(2.0), float(3.0)),
            "req-1",
        ))
        .unwrap(),
    );
    assert_eq!(bad.error().unwrap().kind(), ErrorKind::TypeMismatch);
    assert_eq!(bad.error().unwrap().path().unwrap().to_string(), "size.x");
    let ok = execute(
        &runtime,
        &decode(&invoke_envelope(
            "geom.axis_aligned_box",
            box_input(float(2.0), float(3.0), float(4.0)),
            "req-1",
        ))
        .unwrap(),
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
