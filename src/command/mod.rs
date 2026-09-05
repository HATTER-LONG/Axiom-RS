//! Protocol-independent discovery and invoke commands.
//!
//! Command validates the request envelope only. Capability input contracts are
//! enforced by [`crate::Runtime`].

mod decode;
mod encode;
mod execute;

pub use decode::{COMMAND_VERSION, Command, decode};
pub use encode::CommandResponse;
pub use execute::execute;

use crate::foundation::{Error, Path, Value};

pub(crate) fn request_error(field: &str, message: &str, details: Value) -> Error {
    Error::invalid_request(Path::root().field(field), message, details)
}
