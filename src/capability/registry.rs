//! Single-owner capability registry with atomic conflict handling.

use std::collections::BTreeMap;

use crate::foundation::Error;

use super::descriptor::CapabilityDescriptor;
use super::duplicate_capability;
use super::name::CapabilityName;

/// Authoritative in-process store of capability descriptors.
///
/// The registry starts empty. [`CapabilityRegistry::register`] takes ownership
/// of a descriptor. A duplicate name returns a structured conflict and leaves
/// the previous descriptor unchanged. Discovery methods return owned snapshots.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CapabilityRegistry {
    descriptors: BTreeMap<CapabilityName, CapabilityDescriptor>,
}

impl CapabilityRegistry {
    /// Empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `descriptor` under its name.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorKind::DuplicateCapability`] when the name is already
    /// present. The registry is left unchanged.
    pub fn register(&mut self, descriptor: CapabilityDescriptor) -> Result<(), Error> {
        let name = descriptor.name().clone();
        if self.descriptors.contains_key(&name) {
            return Err(duplicate_capability(name.as_str()));
        }
        self.descriptors.insert(name, descriptor);
        Ok(())
    }

    /// Owned snapshot of the descriptor registered as `name`, if any.
    #[must_use]
    pub fn get(&self, name: &CapabilityName) -> Option<CapabilityDescriptor> {
        self.descriptors.get(name).cloned()
    }

    /// Owned snapshots ordered by [`CapabilityName`] ascending.
    #[must_use]
    pub fn list(&self) -> Vec<CapabilityDescriptor> {
        let mut descriptors: Vec<_> = self.descriptors.values().cloned().collect();
        descriptors.sort_by(|left, right| left.name().cmp(right.name()));
        descriptors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::descriptor::CapabilityDescriptor;
    use crate::capability::name::{CapabilityCategory, CapabilityName};
    use crate::contract::TypeContract;
    use crate::foundation::{ErrorKind, Value};

    fn descriptor(name: &str, description: &str) -> CapabilityDescriptor {
        CapabilityDescriptor::new(
            CapabilityName::parse(name).unwrap(),
            description,
            CapabilityCategory::parse("tool").unwrap(),
            TypeContract::Integer,
            TypeContract::Bool,
        )
        .unwrap()
    }

    #[test]
    fn first_register_succeeds() {
        let mut registry = CapabilityRegistry::new();
        assert!(registry.list().is_empty());
        registry.register(descriptor("echo", "say")).unwrap();
        assert_eq!(registry.list().len(), 1);
        assert_eq!(
            registry
                .get(&CapabilityName::parse("echo").unwrap())
                .unwrap()
                .description(),
            "say"
        );
    }

    #[test]
    fn duplicate_is_atomic_and_structured() {
        let mut registry = CapabilityRegistry::new();
        registry.register(descriptor("echo", "first")).unwrap();
        let error = registry.register(descriptor("echo", "second")).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::DuplicateCapability);
        assert_eq!(
            error
                .details()
                .unwrap()
                .as_object()
                .unwrap()
                .get("capability"),
            Some(&Value::string("echo"))
        );
        assert_eq!(registry.list().len(), 1);
        assert_eq!(
            registry
                .get(&CapabilityName::parse("echo").unwrap())
                .unwrap()
                .description(),
            "first"
        );
    }

    #[test]
    fn names_are_case_sensitive_keys() {
        let mut registry = CapabilityRegistry::new();
        registry.register(descriptor("Echo", "upper")).unwrap();
        registry.register(descriptor("echo", "lower")).unwrap();
        assert_eq!(registry.list().len(), 2);
    }

    #[test]
    fn snapshots_are_isolated_and_sorted() {
        let mut registry = CapabilityRegistry::new();
        registry.register(descriptor("b", "second")).unwrap();
        registry.register(descriptor("a", "first")).unwrap();
        let listed = registry.list();
        let names: Vec<_> = listed.iter().map(|item| item.name().as_str()).collect();
        assert_eq!(names, ["a", "b"]);
        let snapshot = registry.get(&CapabilityName::parse("a").unwrap()).unwrap();
        registry.register(descriptor("c", "third")).unwrap();
        assert_eq!(snapshot.name().as_str(), "a");
        assert_eq!(registry.list().len(), 3);
        assert!(
            registry
                .get(&CapabilityName::parse("missing").unwrap())
                .is_none()
        );
        let mut owned = registry.list();
        owned.clear();
        assert_eq!(registry.list().len(), 3);
    }
}
