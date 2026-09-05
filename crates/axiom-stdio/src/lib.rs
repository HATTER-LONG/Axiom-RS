//! JSON/stdio adapter for Axiom commands.
//!
//! Encoding and framing stay here. Capability contracts are not re-checked.

mod json;
mod serve;

pub use json::{
    AdapterError, MAX_DEPTH, MAX_FRAME_BYTES, decode_value, encode_value, protocol_error_json,
};
pub use serve::serve;
