//! Semantic primitives shared by later runtime layers.
//!
//! This module owns identifiers, dynamic values, diagnostic paths, and
//! structured errors. It does not register capabilities, encode protocols, or
//! execute host code.

mod error;
mod id;
mod path;
mod value;

pub use error::{Error, ErrorKind};
pub use id::{CorrelationId, InvalidIdentifier, MAX_LEN};
pub use path::{Path, PathSegment};
pub use value::{DuplicateField, FiniteFloat, NonFiniteFloat, Object, Value, ValueKind};
