//! Embeddable Axiom Capability Runtime core.
//!
//! Phase 3 publishes sealed type contracts, a thread-affine [`Runtime`] for
//! atomic descriptor-and-implementation registration, synchronous invocation,
//! and a protocol-independent [`command`] boundary. Protocol encoding lives in
//! the `axiom-stdio` crate. [`CapabilityRegistry`] remains a metadata-only
//! catalog; executable discovery uses [`Runtime`].
//!
//! [`TypeContract::object`] is the only public way to build an object contract.
//! List and object variant payloads are crate-private. [`Error`] exposes kind,
//! path, message, and details for observation; owning modules construct
//! failures so callers cannot assemble contradictory errors.
//!
//! # Non-goals
//!
//! Resources, tasks, observers, and additional adapters are out of scope.
//! The core does not depend on serde, JSON, or any transport type.
//!
//! # Module boundaries
//!
//! - [`foundation`]: identifiers, values, paths, and errors
//! - [`contract`]: type shape, field descriptions, and declared constraints
//! - [`execution`]: immutable correlation context
//! - [`capability`]: capability metadata, host trait, and metadata registry
//! - [`runtime`]: executable registration and invocation
//! - [`command`]: list/get/invoke envelope
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
//!     BusinessFailure, Capability, CapabilityCategory, CapabilityDescriptor, CapabilityName,
//!     ExecutionContext, Runtime, TypeContract, Value,
//! };
//!
//! struct Echo;
//! impl Capability for Echo {
//!     fn invoke(
//!         &self,
//!         input: Value,
//!         _context: &ExecutionContext,
//!     ) -> Result<Value, BusinessFailure> {
//!         Ok(input)
//!     }
//! }
//!
//! let runtime = Runtime::new();
//! runtime
//!     .register(
//!         CapabilityDescriptor::new(
//!             CapabilityName::parse("echo").unwrap(),
//!             "return the integer",
//!             CapabilityCategory::parse("tool").unwrap(),
//!             TypeContract::Integer,
//!             TypeContract::Integer,
//!         )
//!         .unwrap(),
//!         Echo,
//!     )
//!     .unwrap();
//! let ctx = ExecutionContext::root(axiom_rs::CorrelationId::parse("req").unwrap());
//! let out = runtime
//!     .invoke(
//!         &CapabilityName::parse("echo").unwrap(),
//!         Value::integer(3),
//!         &ctx,
//!     )
//!     .unwrap();
//! assert_eq!(out, Value::integer(3));
//! ```
//!
//! ```compile_fail
//! fn needs_send<T: Send>(_: T) {}
//! needs_send(axiom_rs::Runtime::new());
//! ```

pub mod capability;
pub mod command;
pub mod contract;
pub mod execution;
pub mod foundation;
pub mod runtime;

pub use crate::capability::{
    BusinessFailure, CAPABILITY_CATEGORY_MAX_LEN, CAPABILITY_NAME_MAX_LEN, Capability,
    CapabilityCategory, CapabilityDescriptor, CapabilityName, CapabilityRegistry,
    InvalidCapabilityCategory, InvalidCapabilityDescriptor, InvalidCapabilityName,
};
pub use crate::command::{
    COMMAND_VERSION, Command, CommandResponse, decode as decode_command, execute,
};
pub use crate::contract::{
    FieldContract, FloatRange, InvalidContract, ListContract, ObjectContract, TypeContract,
};
pub use crate::execution::ExecutionContext;
pub use crate::foundation::{
    CORRELATION_ID_MAX_LEN, CorrelationId, DuplicateField, Error, ErrorKind, FiniteFloat,
    InvalidIdentifier, NonFiniteFloat, Object, Path, PathSegment, Value, ValueKind,
};
pub use crate::runtime::{Runtime, RuntimeHandle};

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
        let _ = crate::Runtime::new();
        let _ = crate::command::COMMAND_VERSION;
    }
}
