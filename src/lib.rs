//! Embeddable Axiom Capability Runtime core.
//!
//! This crate currently publishes Phase 1 semantic primitives and completed
//! Phase 2 capability metadata registration and read-only discovery: validated
//! identifiers, dynamic [`Value`]s, diagnostic [`Path`]s, structured [`Error`]s,
//! type contracts, execution correlation, and capability registration/discovery.
//! Host applications should treat these types as the protocol-independent
//! language of later runtime layers.
//!
//! [`TypeContract::object`] is the only public way to build an object contract.
//! [`Error`] exposes kind, path, message, and details for observation; owning
//! modules construct failures so callers cannot assemble contradictory errors.
//!
//! # Non-goals
//!
//! Resources, tasks, observers, commands, and protocol adapters are out of
//! scope until later phases. The core does not depend on serde, JSON, or any
//! transport type.
//!
//! # Module boundaries
//!
//! - [`foundation`]: identifiers, values, paths, and errors
//! - [`contract`]: type shape description and strict validation
//! - [`execution`]: immutable correlation context
//! - [`capability`]: capability metadata registration and read-only discovery
//!
//! # Examples
//!
//! ```
//! use axiom_rs::{
//!     CorrelationId, ErrorKind, ExecutionContext, FieldContract, Path, TypeContract, Value,
//! };
//!
//! let contract = TypeContract::object(vec![
//!     FieldContract::new("count", TypeContract::Integer, true),
//!     FieldContract::new(
//!         "tags",
//!         TypeContract::list(TypeContract::String),
//!         true,
//!     ),
//! ])
//! .unwrap();
//!
//! let valid = Value::try_object([
//!     ("count", Value::integer(2)),
//!     ("tags", Value::list([Value::from("a"), Value::from("b")])),
//! ])
//! .unwrap();
//! assert!(contract.validate(&valid).is_ok());
//!
//! let invalid = Value::try_object([
//!     ("count", Value::from("two")),
//!     ("tags", Value::list([Value::from("a")])),
//! ])
//! .unwrap();
//! let err = contract.validate(&invalid).unwrap_err();
//! assert_eq!(err.kind(), ErrorKind::TypeMismatch);
//! assert_eq!(err.path(), Some(&Path::root().field("count")));
//!
//! let root = ExecutionContext::root(CorrelationId::parse("req-1").unwrap());
//! let child = root.child(CorrelationId::parse("op-2").unwrap());
//! assert_eq!(child.parent_id(), Some(root.correlation_id()));
//! ```
//!
//! ```
//! use axiom_rs::{
//!     CapabilityCategory, CapabilityDescriptor, CapabilityName, CapabilityRegistry, ErrorKind,
//!     TypeContract, Value,
//! };
//!
//! let input = TypeContract::object(vec![axiom_rs::FieldContract::new(
//!     "text",
//!     TypeContract::String,
//!     true,
//! )])
//! .unwrap();
//! let echo = CapabilityDescriptor::new(
//!     CapabilityName::parse("echo").unwrap(),
//!     "return the text",
//!     CapabilityCategory::parse("tool").unwrap(),
//!     input,
//!     TypeContract::String,
//! )
//! .unwrap();
//! let list = CapabilityDescriptor::new(
//!     CapabilityName::parse("list").unwrap(),
//!     "list items",
//!     CapabilityCategory::parse("tool").unwrap(),
//!     TypeContract::Null,
//!     TypeContract::list(TypeContract::String),
//! )
//! .unwrap();
//!
//! let mut registry = CapabilityRegistry::new();
//! registry.register(echo).unwrap();
//! registry.register(list).unwrap();
//! let listed = registry.list();
//! let names: Vec<_> = listed.iter().map(|item| item.name().as_str()).collect();
//! assert_eq!(names, ["echo", "list"]);
//!
//! let discovered = registry.get(&CapabilityName::parse("echo").unwrap()).unwrap();
//! let value = axiom_rs::Value::try_object([("text", Value::from("hi"))]).unwrap();
//! assert!(discovered.input().validate(&value).is_ok());
//!
//! let err = registry.register(discovered).unwrap_err();
//! assert_eq!(err.kind(), ErrorKind::DuplicateCapability);
//! assert!(registry.get(&CapabilityName::parse("missing").unwrap()).is_none());
//! ```

pub mod capability;
pub mod contract;
pub mod execution;
pub mod foundation;

pub use crate::capability::{
    CAPABILITY_CATEGORY_MAX_LEN, CAPABILITY_NAME_MAX_LEN, CapabilityCategory, CapabilityDescriptor,
    CapabilityName, CapabilityRegistry, InvalidCapabilityCategory, InvalidCapabilityDescriptor,
    InvalidCapabilityName,
};
pub use crate::contract::{FieldContract, InvalidContract, TypeContract};
pub use crate::execution::ExecutionContext;
pub use crate::foundation::{
    CORRELATION_ID_MAX_LEN, CorrelationId, DuplicateField, Error, ErrorKind, FiniteFloat,
    InvalidIdentifier, NonFiniteFloat, Object, Path, PathSegment, Value, ValueKind,
};

#[cfg(test)]
mod tests {
    #[test]
    fn public_modules_are_present() {
        let _ = crate::foundation::CorrelationId::parse("ok");
        let _ = crate::contract::TypeContract::Bool;
        let _ = crate::execution::ExecutionContext::root(
            crate::foundation::CorrelationId::parse("root").unwrap(),
        );
        let _ = crate::capability::duplicate_capability("echo");
    }
}
