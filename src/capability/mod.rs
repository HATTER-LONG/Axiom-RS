//! Capability metadata, implementation boundary, and metadata-only registry.
//!
//! Executable registration lives on [`crate::Runtime`]. [`CapabilityRegistry`]
//! remains a metadata catalog; it does not store implementations and is not
//! the authority for invoke/discover of executable capabilities.

mod descriptor;
mod invoke;
mod label;
mod name;
mod registry;

use crate::foundation::Error;

pub use descriptor::{CapabilityDescriptor, InvalidCapabilityDescriptor};
pub use invoke::{BusinessFailure, Capability};
pub use name::{
    CAPABILITY_CATEGORY_MAX_LEN, CAPABILITY_NAME_MAX_LEN, CapabilityCategory, CapabilityName,
    InvalidCapabilityCategory, InvalidCapabilityName,
};
pub use registry::CapabilityRegistry;

/// Structured failure for a capability name that is already registered.
#[must_use]
pub(crate) fn duplicate_capability(name: impl Into<String>) -> Error {
    Error::duplicate_capability(name)
}

#[cfg(test)]
mod tests {
    use super::duplicate_capability;
    use crate::foundation::{ErrorKind, Value};

    #[test]
    fn duplicate_kind_is_stable_without_parsing_message() {
        let error = duplicate_capability("echo");
        assert_eq!(error.kind(), ErrorKind::DuplicateCapability);
        assert_eq!(error.kind().as_str(), "duplicate_capability");
        assert!(error.path().is_none());
        assert_eq!(
            error
                .details()
                .and_then(Value::as_object)
                .and_then(|fields| fields.get("capability").cloned()),
            Some(Value::string("echo"))
        );
        assert_ne!(error.kind(), ErrorKind::InvalidIdentifier);
    }
}
