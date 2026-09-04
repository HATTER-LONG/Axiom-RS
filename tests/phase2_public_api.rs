//! Black-box checks of capability metadata, registration, and discovery.

use axiom_rs::{
    CAPABILITY_CATEGORY_MAX_LEN, CAPABILITY_NAME_MAX_LEN, CapabilityCategory, CapabilityDescriptor,
    CapabilityName, CapabilityRegistry, CorrelationId, ErrorKind, FieldContract,
    InvalidCapabilityCategory, InvalidCapabilityDescriptor, InvalidCapabilityName, TypeContract,
    Value,
};

fn as_std_error<T: std::error::Error>(error: &T) -> &dyn std::error::Error {
    error
}

fn descriptor(name: &str, description: &str) -> CapabilityDescriptor {
    CapabilityDescriptor::new(
        CapabilityName::parse(name).unwrap(),
        description,
        CapabilityCategory::parse("tool").unwrap(),
        TypeContract::object(vec![FieldContract::new("text", TypeContract::String, true)]).unwrap(),
        TypeContract::String,
    )
    .unwrap()
}

#[test]
fn names_are_not_correlation_ids() {
    let name = CapabilityName::parse("req-1").unwrap();
    let id = CorrelationId::parse("req-1").unwrap();
    assert_eq!(name.as_str(), id.as_str());
    assert_eq!(name.as_ref(), "req-1");
}

#[test]
fn category_max_len_is_independent_of_name() {
    assert_eq!(CAPABILITY_NAME_MAX_LEN, 128);
    assert_eq!(CAPABILITY_CATEGORY_MAX_LEN, 64);
    let name_only = "a".repeat(65);
    assert!(CapabilityName::parse(&name_only).is_ok());
    assert!(matches!(
        CapabilityCategory::parse(&name_only).unwrap_err(),
        InvalidCapabilityCategory::TooLong { length: 65 }
    ));
}

#[test]
fn construction_errors_are_typed_and_std_error() {
    let _ = as_std_error(&CapabilityName::parse("").unwrap_err());
    let _ = as_std_error(&CapabilityCategory::parse(" ").unwrap_err());
    let _ = as_std_error(&InvalidCapabilityDescriptor::EmptyDescription);
    assert_eq!(
        CapabilityName::parse("").unwrap_err(),
        InvalidCapabilityName::Empty
    );
    assert_eq!(
        CapabilityDescriptor::new(
            CapabilityName::parse("echo").unwrap(),
            "   ",
            CapabilityCategory::parse("tool").unwrap(),
            TypeContract::Null,
            TypeContract::Null,
        )
        .unwrap_err(),
        InvalidCapabilityDescriptor::EmptyDescription
    );
}

#[test]
fn register_discover_and_reuse_input_contract() {
    let mut registry = CapabilityRegistry::new();
    assert!(registry.list().is_empty());
    registry.register(descriptor("list", "later")).unwrap();
    registry.register(descriptor("echo", "first")).unwrap();

    let listed = registry.list();
    let names: Vec<_> = listed.iter().map(|item| item.name().as_str()).collect();
    assert_eq!(names, ["echo", "list"]);

    let echo = registry
        .get(&CapabilityName::parse("echo").unwrap())
        .unwrap();
    let valid = Value::try_object([("text", Value::from("hi"))]).unwrap();
    assert!(echo.input().validate(&valid).is_ok());
    let invalid = Value::try_object([("text", Value::integer(1))]).unwrap();
    assert_eq!(
        echo.input().validate(&invalid).unwrap_err().kind(),
        ErrorKind::TypeMismatch
    );

    let err = registry.register(descriptor("echo", "other")).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DuplicateCapability);
    assert_eq!(
        err.details()
            .unwrap()
            .as_object()
            .unwrap()
            .get("capability"),
        Some(&Value::string("echo"))
    );
    assert!(
        registry
            .get(&CapabilityName::parse("missing").unwrap())
            .is_none()
    );

    let snapshot = registry.list();
    registry.register(descriptor("zeta", "third")).unwrap();
    assert_eq!(snapshot.len(), 2);
    assert_eq!(registry.list().len(), 3);
}
