//! Black-box checks of the Phase 1 public API.

use axiom_rs::{
    CORRELATION_ID_MAX_LEN, CorrelationId, DuplicateField, ErrorKind, ExecutionContext,
    FieldContract, InvalidContract, InvalidIdentifier, NonFiniteFloat, Path, TypeContract, Value,
};

fn as_std_error<T: std::error::Error>(error: &T) -> &dyn std::error::Error {
    error
}

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

#[test]
fn object_contract_rejects_empty_and_duplicate_fields() {
    assert_eq!(
        TypeContract::object(vec![FieldContract::new("", TypeContract::Null, true)]).unwrap_err(),
        InvalidContract::EmptyFieldName
    );
    assert_eq!(
        TypeContract::object(vec![
            FieldContract::new("a", TypeContract::Bool, true),
            FieldContract::new("a", TypeContract::Integer, false),
        ])
        .unwrap_err(),
        InvalidContract::DuplicateField { name: "a".into() }
    );
}

#[test]
fn public_operations_produce_phase1_error_kinds() {
    let invalid_identifier = axiom_rs::Error::from(CorrelationId::parse("").unwrap_err());
    assert_eq!(invalid_identifier.kind(), ErrorKind::InvalidIdentifier);

    let contract = TypeContract::object(vec![
        FieldContract::new("need", TypeContract::Integer, true),
        FieldContract::new("keep", TypeContract::Bool, true),
    ])
    .unwrap();

    let missing = contract
        .validate(&Value::try_object([("keep", Value::from(true))]).unwrap())
        .unwrap_err();
    assert_eq!(missing.kind(), ErrorKind::MissingField);

    let unknown = contract
        .validate(
            &Value::try_object([
                ("need", Value::integer(1)),
                ("keep", Value::from(true)),
                ("extra", Value::null()),
            ])
            .unwrap(),
        )
        .unwrap_err();
    assert_eq!(unknown.kind(), ErrorKind::UnknownField);

    let mismatch = TypeContract::Integer
        .validate(&Value::from("x"))
        .unwrap_err();
    assert_eq!(mismatch.kind(), ErrorKind::TypeMismatch);
}

#[test]
fn construction_errors_implement_std_error() {
    let _ = as_std_error(&CorrelationId::parse("").unwrap_err());
    let _ = as_std_error(
        &TypeContract::object(vec![FieldContract::new("", TypeContract::Null, true)]).unwrap_err(),
    );
    let _ =
        as_std_error(&Value::try_object([("a", Value::null()), ("a", Value::null())]).unwrap_err());
    let _ = as_std_error(&Value::try_float(f64::INFINITY).unwrap_err());
    let _ = as_std_error(
        &TypeContract::Integer
            .validate(&Value::from("x"))
            .unwrap_err(),
    );
    let _: InvalidIdentifier;
    let _: InvalidContract;
    let _: DuplicateField;
    let _: NonFiniteFloat;
}

#[test]
fn correlation_id_as_ref_matches_stored_text() {
    let id = CorrelationId::parse("Req-1").unwrap();
    assert_eq!(id.as_ref(), "Req-1");
    assert_eq!(id.as_ref(), id.as_str());
    assert_eq!(CORRELATION_ID_MAX_LEN, 128);
    assert!(CorrelationId::parse("a".repeat(CORRELATION_ID_MAX_LEN)).is_ok());
    assert!(CorrelationId::parse("a".repeat(CORRELATION_ID_MAX_LEN + 1)).is_err());
}

#[test]
fn root_path_is_omitted_from_error_display() {
    let err = TypeContract::Integer
        .validate(&Value::from("x"))
        .unwrap_err();
    assert_eq!(err.path(), Some(&Path::root()));
    assert!(err.path().unwrap().segments().is_empty());
    assert_eq!(
        err.to_string(),
        "type_mismatch: expected integer, found string"
    );
}
