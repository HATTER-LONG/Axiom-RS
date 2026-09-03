//! Embeddable Axiom Capability Runtime core.
//!
//! This crate currently publishes only Phase 1 semantic primitives: validated
//! identifiers, dynamic [`Value`]s, diagnostic [`Path`]s, structured [`Error`]s,
//! type contracts, and execution correlation. Host applications should treat
//! these types as the protocol-independent language of later runtime layers.
//!
//! # Non-goals
//!
//! Capability registration, resources, tasks, observers, commands, and protocol
//! adapters are out of scope until later phases. The core does not depend on
//! serde, JSON, or any transport type.
//!
//! # Module boundaries
//!
//! - [`foundation`]: identifiers, values, paths, and errors
//! - [`contract`]: type shape description and strict validation
//! - [`execution`]: immutable correlation context
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

pub mod contract;
pub mod execution;
pub mod foundation;

pub use crate::contract::{FieldContract, InvalidContract, TypeContract};
pub use crate::execution::ExecutionContext;
pub use crate::foundation::{
    CorrelationId, DuplicateField, Error, ErrorKind, FiniteFloat, InvalidIdentifier, MAX_LEN,
    NonFiniteFloat, Object, Path, PathSegment, Value, ValueKind,
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
    }
}
