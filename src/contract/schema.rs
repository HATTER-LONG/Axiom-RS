//! Runtime-readable description of an expected [`Value`] shape.

use crate::foundation::ValueKind;

/// Why a type contract could not be constructed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvalidContract {
    /// Object field name was empty.
    EmptyFieldName,
    /// The same object field name was declared twice.
    DuplicateField {
        /// Repeated name.
        name: String,
    },
}

impl std::fmt::Display for InvalidContract {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyFieldName => f.write_str("object field name must not be empty"),
            Self::DuplicateField { name } => write!(f, "duplicate contract field {name}"),
        }
    }
}

impl std::error::Error for InvalidContract {}

/// Named field inside an object contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldContract {
    name: String,
    contract: TypeContract,
    required: bool,
}

impl FieldContract {
    /// Declare a field. `name` must be non-empty; uniqueness is checked by
    /// [`TypeContract::object`].
    #[must_use]
    pub fn new(name: impl Into<String>, contract: TypeContract, required: bool) -> Self {
        Self {
            name: name.into(),
            contract,
            required,
        }
    }

    /// Field name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Nested contract for this field.
    #[must_use]
    pub fn contract(&self) -> &TypeContract {
        &self.contract
    }

    /// Whether a valid object must include this field.
    #[must_use]
    pub fn required(&self) -> bool {
        self.required
    }
}

/// Expected shape of a [`crate::Value`].
///
/// Contracts are data, not Rust `TypeId`s. Object field order is the order
/// supplied to [`TypeContract::object`]. Objects are strict: undeclared fields
/// are rejected during validation.
///
/// Composite object contracts are only constructed through
/// [`TypeContract::object`]. Direct variant construction cannot produce an
/// object with an empty or duplicate field name.
///
/// ```compile_fail
/// use axiom_rs::{FieldContract, TypeContract};
/// let _ = TypeContract::Object {
///     fields: vec![FieldContract::new("", TypeContract::Null, true)],
/// };
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeContract {
    /// Null scalar.
    Null,
    /// Boolean scalar.
    Bool,
    /// Integer scalar.
    Integer,
    /// Finite float scalar.
    Float,
    /// String scalar.
    String,
    /// Homogeneous list.
    List {
        /// Contract each item must satisfy.
        item: Box<TypeContract>,
    },
    /// Strict object. Created only by [`TypeContract::object`].
    #[non_exhaustive]
    Object {
        /// Fields in declaration order.
        fields: Vec<FieldContract>,
    },
}

impl TypeContract {
    /// List whose items match `item`.
    #[must_use]
    pub fn list(item: TypeContract) -> Self {
        Self::List {
            item: Box::new(item),
        }
    }

    /// Strict object contract.
    ///
    /// # Errors
    ///
    /// Fails when a field name is empty or repeated.
    pub fn object(fields: Vec<FieldContract>) -> Result<Self, InvalidContract> {
        let mut seen = std::collections::HashSet::new();
        for field in &fields {
            if field.name.is_empty() {
                return Err(InvalidContract::EmptyFieldName);
            }
            if !seen.insert(field.name.clone()) {
                return Err(InvalidContract::DuplicateField {
                    name: field.name.clone(),
                });
            }
        }
        Ok(Self::Object { fields })
    }

    /// Kind this contract expects at the current node.
    #[must_use]
    pub fn value_kind(&self) -> ValueKind {
        match self {
            Self::Null => ValueKind::Null,
            Self::Bool => ValueKind::Bool,
            Self::Integer => ValueKind::Integer,
            Self::Float => ValueKind::Float,
            Self::String => ValueKind::String,
            Self::List { .. } => ValueKind::List,
            Self::Object { .. } => ValueKind::Object,
        }
    }

    /// Object fields in declaration order.
    #[must_use]
    pub fn fields(&self) -> Option<&[FieldContract]> {
        match self {
            Self::Object { fields } => Some(fields),
            _ => None,
        }
    }

    /// List item contract.
    #[must_use]
    pub fn item(&self) -> Option<&TypeContract> {
        match self {
            Self::List { item } => Some(item),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_nested_contract() {
        let contract = TypeContract::object(vec![
            FieldContract::new("count", TypeContract::Integer, true),
            FieldContract::new("tags", TypeContract::list(TypeContract::String), false),
            FieldContract::new(
                "child",
                TypeContract::object(vec![FieldContract::new("ok", TypeContract::Bool, true)])
                    .unwrap(),
                true,
            ),
        ])
        .unwrap();
        let names: Vec<_> = contract
            .fields()
            .unwrap()
            .iter()
            .map(FieldContract::name)
            .collect();
        assert_eq!(names, ["count", "tags", "child"]);
        assert_eq!(contract.value_kind(), ValueKind::Object);
        assert!(contract.item().is_none());
        let tags = &contract.fields().unwrap()[1];
        assert_eq!(tags.contract().item(), Some(&TypeContract::String));
        assert_eq!(
            TypeContract::list(TypeContract::Integer).item(),
            Some(&TypeContract::Integer)
        );
    }

    #[test]
    fn rejects_duplicate_fields() {
        let err = TypeContract::object(vec![
            FieldContract::new("a", TypeContract::Null, true),
            FieldContract::new("a", TypeContract::Bool, false),
        ])
        .unwrap_err();
        assert_eq!(err, InvalidContract::DuplicateField { name: "a".into() });
    }

    #[test]
    fn rejects_empty_field_name() {
        let err = TypeContract::object(vec![FieldContract::new("", TypeContract::Null, true)])
            .unwrap_err();
        assert_eq!(err, InvalidContract::EmptyFieldName);
        assert!(!err.to_string().is_empty());
        let _: &dyn std::error::Error = &err;
        assert!(
            !InvalidContract::DuplicateField { name: "a".into() }
                .to_string()
                .is_empty()
        );
    }

    #[test]
    fn empty_object_is_allowed() {
        let contract = TypeContract::object(vec![]).unwrap();
        assert!(contract.fields().unwrap().is_empty());
    }
}
