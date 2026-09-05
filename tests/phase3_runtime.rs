//! Runtime reentry, context, and nested input diagnostics.

use axiom_rs::{
    BusinessFailure, Capability, CapabilityCategory, CapabilityDescriptor, CapabilityName,
    CorrelationId, ErrorKind, ExecutionContext, FieldContract, Path, Runtime, RuntimeHandle,
    TypeContract, Value,
};

struct Inner;

impl Capability for Inner {
    fn invoke(&self, input: Value, context: &ExecutionContext) -> Result<Value, BusinessFailure> {
        assert_eq!(context.correlation_id().as_str(), "child");
        assert_eq!(context.parent_id().map(CorrelationId::as_str), Some("root"));
        Ok(input)
    }
}

struct Outer {
    runtime: RuntimeHandle,
}

impl Capability for Outer {
    fn invoke(&self, _input: Value, context: &ExecutionContext) -> Result<Value, BusinessFailure> {
        let listed = self.runtime.runtime().list();
        assert!(listed.iter().any(|item| item.name().as_str() == "inner"));
        let child = context.child(CorrelationId::parse("child").unwrap());
        self.runtime
            .runtime()
            .invoke(
                &CapabilityName::parse("inner").unwrap(),
                Value::integer(4),
                &child,
            )
            .map_err(|err| BusinessFailure::new(err.message()))
    }
}

fn descriptor(name: &str, input: TypeContract, output: TypeContract) -> CapabilityDescriptor {
    CapabilityDescriptor::new(
        CapabilityName::parse(name).unwrap(),
        "reentry test",
        CapabilityCategory::parse("test").unwrap(),
        input,
        output,
    )
    .unwrap()
}

#[test]
fn host_reenters_discover_and_invoke() {
    let runtime = Runtime::new();
    runtime
        .register(
            descriptor("inner", TypeContract::Integer, TypeContract::Integer),
            Inner,
        )
        .unwrap();
    runtime
        .register(
            descriptor("outer", TypeContract::Null, TypeContract::Integer),
            Outer {
                runtime: runtime.handle(),
            },
        )
        .unwrap();
    let root = ExecutionContext::root(CorrelationId::parse("root").unwrap());
    let out = runtime
        .invoke(
            &CapabilityName::parse("outer").unwrap(),
            Value::null(),
            &root,
        )
        .unwrap();
    assert_eq!(out, Value::integer(4));
}

#[test]
fn nested_input_error_keeps_path() {
    let runtime = Runtime::new();
    let input = TypeContract::object(vec![FieldContract::new(
        "child",
        TypeContract::object(vec![FieldContract::new("n", TypeContract::Integer, true)]).unwrap(),
        true,
    )])
    .unwrap();
    runtime
        .register(descriptor("nested", input, TypeContract::Null), Inner)
        .unwrap();
    let value = Value::try_object([(
        "child",
        Value::try_object([("n", Value::from("x"))]).unwrap(),
    )])
    .unwrap();
    let err = runtime
        .invoke(
            &CapabilityName::parse("nested").unwrap(),
            value,
            &ExecutionContext::root(CorrelationId::parse("r").unwrap()),
        )
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::TypeMismatch);
    assert_eq!(err.path(), Some(&Path::root().field("child").field("n")));
}
