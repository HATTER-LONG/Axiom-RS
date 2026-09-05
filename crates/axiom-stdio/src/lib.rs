//! JSON/stdio adapter for Axiom commands.
//!
//! Encoding and framing stay here. Capability contracts are not re-checked.
//!
//! [`MAX_DEPTH`] is Axiom [`axiom_rs::Value`] nesting. Integer, float, and
//! escaped-object JSON wrappers do not add a Value level. Request lines are
//! limited to [`MAX_FRAME_BYTES`] while reading; oversized input is rejected
//! and the rest of that line is discarded so the next request can be served.

mod json;
mod serve;

pub use json::{
    AdapterError, MAX_DEPTH, MAX_FRAME_BYTES, decode_value, encode_value, protocol_error_json,
};
pub use serve::serve;
