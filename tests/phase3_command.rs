//! Command envelope tests from outside the crate.

use axiom_rs::{
    COMMAND_VERSION, Capability, CapabilityCategory, CapabilityDescriptor, CapabilityName, Command,
    CorrelationId, ErrorKind, ExecutionContext, Runtime, TypeContract, Value, decode_command,
    execute,
};

struct Echo;

impl Capability for Echo {
    fn invoke(
        &self,
        input: Value,
        context: &ExecutionContext,
    ) -> Result<Value, axiom_rs::BusinessFailure> {
        assert_eq!(context.correlation_id().as_str(), "id");
        Ok(input)
    }
}

#[test]
fn command_and_native_invoke_agree() {
    let runtime = Runtime::new();
    runtime
        .register(
            CapabilityDescriptor::new(
                CapabilityName::parse("echo").unwrap(),
                "echo",
                CapabilityCategory::parse("t").unwrap(),
                TypeContract::Integer,
                TypeContract::Integer,
            )
            .unwrap(),
            Echo,
        )
        .unwrap();
    let request = Value::try_object([
        ("v", Value::integer(COMMAND_VERSION)),
        ("cmd", Value::string("invoke")),
        ("name", Value::string("echo")),
        ("input", Value::integer(5)),
        ("correlation_id", Value::string("id")),
        ("parent_id", Value::string("p")),
    ])
    .unwrap();
    let command = decode_command(&request).unwrap();
    match &command {
        Command::Invoke { context, .. } => {
            assert_eq!(context.parent_id().map(CorrelationId::as_str), Some("p"));
        }
        _ => panic!("expected invoke"),
    }
    let via_command = execute(&runtime, &command);
    let via_native = runtime
        .invoke(
            &CapabilityName::parse("echo").unwrap(),
            Value::integer(5),
            &ExecutionContext::root(CorrelationId::parse("p").unwrap())
                .child(CorrelationId::parse("id").unwrap()),
        )
        .unwrap();
    assert_eq!(via_command.value(), Some(&via_native));
}

#[test]
fn unknown_command_does_not_touch_runtime() {
    let err = decode_command(
        &Value::try_object([("v", Value::integer(1)), ("cmd", Value::string("explode"))]).unwrap(),
    )
    .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownCommand);
}

#[test]
fn exported_paths_distinguish_root_and_absence() {
    let root = TypeContract::Integer
        .validate(&Value::string("n"))
        .unwrap_err();
    assert_eq!(root.path(), Some(&axiom_rs::Path::root()));
    let encoded = axiom_rs::CommandResponse::from_error(root).to_value();
    let path = encoded
        .as_object()
        .unwrap()
        .get("error")
        .unwrap()
        .as_object()
        .unwrap()
        .get("path")
        .unwrap()
        .as_object()
        .unwrap();
    assert!(path.get("segments").unwrap().as_list().unwrap().is_empty());
    let unknown = decode_command(
        &Value::try_object([
            ("v", Value::integer(1)),
            ("cmd", Value::string("invoke")),
            ("name", Value::string("gone")),
            ("input", Value::null()),
            ("correlation_id", Value::string("c")),
        ])
        .unwrap(),
    )
    .unwrap();
    let runtime = Runtime::new();
    let response = execute(&runtime, &unknown);
    assert!(response.error().unwrap().path().is_none());
    assert_eq!(
        response
            .to_value()
            .as_object()
            .unwrap()
            .get("error")
            .unwrap()
            .as_object()
            .unwrap()
            .get("path"),
        Some(&Value::null())
    );
}
