//! Type contracts and strict validation of [`crate::Value`].

mod schema;
mod validate;

pub use schema::{FieldContract, InvalidContract, TypeContract};
