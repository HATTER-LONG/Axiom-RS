//! Immutable capability metadata.

use crate::contract::TypeContract;

use super::name::{CapabilityCategory, CapabilityName};

/// Runtime-readable description of a capability's name, category, and I/O contracts.
///
/// A descriptor is created through [`CapabilityDescriptor::new`] and is immutable
/// afterwards. Input and output reuse [`TypeContract`] without copying its
/// validation rules.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityDescriptor {
    name: CapabilityName,
    description: String,
    category: CapabilityCategory,
    input: TypeContract,
    output: TypeContract,
}

/// Why a [`CapabilityDescriptor`] could not be constructed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvalidCapabilityDescriptor {
    /// Description was empty or contained only whitespace.
    EmptyDescription,
}

impl CapabilityDescriptor {
    /// Assemble an immutable descriptor from validated metadata and contracts.
    ///
    /// `description` is stored exactly as supplied when it contains at least one
    /// non-whitespace character.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidCapabilityDescriptor::EmptyDescription`] when
    /// `description` is empty or only whitespace.
    pub fn new(
        name: CapabilityName,
        description: impl Into<String>,
        category: CapabilityCategory,
        input: TypeContract,
        output: TypeContract,
    ) -> Result<Self, InvalidCapabilityDescriptor> {
        let description = description.into();
        if is_blank(&description) {
            return Err(InvalidCapabilityDescriptor::EmptyDescription);
        }
        Ok(Self {
            name,
            description,
            category,
            input,
            output,
        })
    }

    /// Capability name.
    #[must_use]
    pub fn name(&self) -> &CapabilityName {
        &self.name
    }

    /// Original description text.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Encode this descriptor for Command discovery.
    #[must_use]
    pub fn to_value(&self) -> crate::foundation::Value {
        crate::foundation::Value::try_object([
            ("name", crate::foundation::Value::string(self.name.as_str())),
            (
                "description",
                crate::foundation::Value::string(self.description.clone()),
            ),
            (
                "category",
                crate::foundation::Value::string(self.category.as_str()),
            ),
            ("input", self.input.to_value()),
            ("output", self.output.to_value()),
        ])
        .expect("descriptor keys are unique")
    }

    /// Capability category.
    #[must_use]
    pub fn category(&self) -> &CapabilityCategory {
        &self.category
    }

    /// Input contract.
    #[must_use]
    pub fn input(&self) -> &TypeContract {
        &self.input
    }

    /// Output contract.
    #[must_use]
    pub fn output(&self) -> &TypeContract {
        &self.output
    }
}

fn is_blank(value: &str) -> bool {
    value.is_empty() || value.chars().all(char::is_whitespace)
}

impl std::fmt::Display for InvalidCapabilityDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyDescription => {
                f.write_str("capability description must not be empty or blank")
            }
        }
    }
}

impl std::error::Error for InvalidCapabilityDescriptor {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{FieldContract, TypeContract};

    fn sample(
        name: &str,
        description: &str,
        category: &str,
    ) -> Result<CapabilityDescriptor, InvalidCapabilityDescriptor> {
        CapabilityDescriptor::new(
            CapabilityName::parse(name).unwrap(),
            description,
            CapabilityCategory::parse(category).unwrap(),
            TypeContract::object(vec![FieldContract::new("n", TypeContract::Integer, true)])
                .unwrap(),
            TypeContract::Bool,
        )
    }

    #[test]
    fn accepts_legal_metadata_and_preserves_fields() {
        let descriptor = sample("math.add", " Add two integers. ", "math").unwrap();
        assert_eq!(descriptor.name().as_str(), "math.add");
        assert_eq!(descriptor.description(), " Add two integers. ");
        assert_eq!(descriptor.category().as_str(), "math");
        assert_eq!(descriptor.input().fields().unwrap()[0].name(), "n");
        assert_eq!(descriptor.output(), &TypeContract::Bool);
    }

    #[test]
    fn rejects_empty_and_whitespace_descriptions() {
        assert_eq!(
            sample("echo", "", "tool").unwrap_err(),
            InvalidCapabilityDescriptor::EmptyDescription
        );
        assert_eq!(
            sample("echo", " \t\n", "tool").unwrap_err(),
            InvalidCapabilityDescriptor::EmptyDescription
        );
        let _: &dyn std::error::Error = &InvalidCapabilityDescriptor::EmptyDescription;
        assert!(
            !InvalidCapabilityDescriptor::EmptyDescription
                .to_string()
                .is_empty()
        );
    }

    #[test]
    fn clone_is_isolated_from_later_local_changes() {
        let original = sample("echo", "say hello", "tool").unwrap();
        let cloned = original.clone();
        drop(original);
        assert_eq!(cloned.description(), "say hello");
        assert_eq!(cloned.name().as_str(), "echo");
    }
}
