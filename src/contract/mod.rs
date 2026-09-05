//! Type contracts and strict validation of [`crate::Value`].

mod export;
mod schema;
mod validate;

pub use schema::{
    FieldContract, FloatRange, InvalidContract, ListContract, ObjectContract, TypeContract,
};
