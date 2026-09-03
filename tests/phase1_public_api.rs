//! Black-box checks of the Phase 1 public API.

use axiom_rs::{
    CorrelationId, ErrorKind, ExecutionContext, FieldContract, Path, TypeContract, Value,
};

#[test]
fn validates_nested_value_and_propagates_correlation() {
    let contract = TypeContract::object(vec![
        FieldContract::new("enabled", TypeContract::Bool, true),
        FieldContract::new(
            "mesh",
            TypeContract::object(vec![FieldContract::new(
                "faces",
                TypeContract::list(
                    TypeContract::object(vec![FieldContract::new(
                        "size",
                        TypeContract::Integer,
                        true,
                    )])
                    .unwrap(),
                ),
                true,
            )])
            .unwrap(),
            true,
        ),
    ])
    .unwrap();

    let valid = Value::try_object([
        ("enabled", Value::from(true)),
        (
            "mesh",
            Value::try_object([(
                "faces",
                Value::list([Value::try_object([("size", Value::integer(3))]).unwrap()]),
            )])
            .unwrap(),
        ),
    ])
    .unwrap();
    assert!(contract.validate(&valid).is_ok());

    let invalid = Value::try_object([
        ("enabled", Value::from(true)),
        (
            "mesh",
            Value::try_object([(
                "faces",
                Value::list([Value::try_object([("size", Value::from("big"))]).unwrap()]),
            )])
            .unwrap(),
        ),
    ])
    .unwrap();
    let err = contract.validate(&invalid).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::TypeMismatch);
    assert_eq!(
        err.path(),
        Some(
            &Path::root()
                .field("mesh")
                .field("faces")
                .index(0)
                .field("size")
        )
    );

    let root = ExecutionContext::root(CorrelationId::parse("req-1").unwrap());
    let child = root.child(CorrelationId::parse("face-0").unwrap());
    assert_eq!(child.parent_id(), Some(root.correlation_id()));
}
