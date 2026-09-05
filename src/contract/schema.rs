//! Runtime-readable description of an expected [`Value`] shape.

use crate::foundation::{FiniteFloat, ValueKind};

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
    /// Field description was empty or blank.
    EmptyDescription,
    /// Unit label was empty or blank.
    EmptyUnit,
    /// Enum constraint had no values.
    EmptyEnum,
    /// An enum value was empty or blank.
    EmptyEnumValue,
    /// An enum value was repeated.
    DuplicateEnumValue {
        /// Repeated value.
        value: String,
    },
    /// Enum constraint applied to a non-string field.
    EnumOnNonString,
    /// Integer range applied to a non-integer field.
    IntegerRangeOnNonInteger,
    /// Float range applied to a non-float field.
    FloatRangeOnNonFloat,
    /// Integer minimum was greater than maximum.
    InvalidIntegerRange,
    /// Float minimum was greater than maximum.
    InvalidFloatRange,
    /// Float bound was NaN or infinite.
    NonFiniteFloatBound,
}

impl std::fmt::Display for InvalidContract {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyFieldName => f.write_str("object field name must not be empty"),
            Self::DuplicateField { name } => write!(f, "duplicate contract field {name}"),
            Self::EmptyDescription => f.write_str("field description must not be empty"),
            Self::EmptyUnit => f.write_str("field unit must not be empty"),
            Self::EmptyEnum => f.write_str("enum constraint must include a value"),
            Self::EmptyEnumValue => f.write_str("enum value must not be empty"),
            Self::DuplicateEnumValue { value } => write!(f, "duplicate enum value {value}"),
            Self::EnumOnNonString => f.write_str("enum constraint requires a string field"),
            Self::IntegerRangeOnNonInteger => {
                f.write_str("integer range requires an integer field")
            }
            Self::FloatRangeOnNonFloat => f.write_str("float range requires a float field"),
            Self::InvalidIntegerRange => f.write_str("integer minimum is greater than maximum"),
            Self::InvalidFloatRange => f.write_str("float minimum is greater than maximum"),
            Self::NonFiniteFloatBound => f.write_str("float bound must be finite"),
        }
    }
}

impl std::error::Error for InvalidContract {}

/// Inclusive or exclusive finite float bounds for a float field.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FloatRange {
    /// Minimum bound, if any.
    pub min: Option<f64>,
    /// Maximum bound, if any.
    pub max: Option<f64>,
    /// When true, values equal to `min` are rejected.
    pub min_exclusive: bool,
    /// When true, values equal to `max` are rejected.
    pub max_exclusive: bool,
}

/// Named field inside an object contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldContract {
    name: String,
    contract: TypeContract,
    required: bool,
    description: String,
    unit: Option<String>,
    enum_values: Option<Vec<String>>,
    integer_min: Option<i64>,
    integer_max: Option<i64>,
    float_min: Option<FiniteFloat>,
    float_max: Option<FiniteFloat>,
    float_min_exclusive: bool,
    float_max_exclusive: bool,
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
            description: String::new(),
            unit: None,
            enum_values: None,
            integer_min: None,
            integer_max: None,
            float_min: None,
            float_max: None,
            float_min_exclusive: false,
            float_max_exclusive: false,
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

    /// Human-readable field description, when declared.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        if self.description.is_empty() {
            None
        } else {
            Some(self.description.as_str())
        }
    }

    /// Unit label for discovery, when declared.
    #[must_use]
    pub fn unit(&self) -> Option<&str> {
        self.unit.as_deref()
    }

    /// Allowed string values, when an enum constraint is declared.
    #[must_use]
    pub fn enum_values(&self) -> Option<&[String]> {
        self.enum_values.as_deref()
    }

    /// Inclusive integer minimum, when declared.
    #[must_use]
    pub fn integer_min(&self) -> Option<i64> {
        self.integer_min
    }

    /// Inclusive integer maximum, when declared.
    #[must_use]
    pub fn integer_max(&self) -> Option<i64> {
        self.integer_max
    }

    /// Float minimum bound, when declared.
    #[must_use]
    pub fn float_min(&self) -> Option<f64> {
        self.float_min.map(FiniteFloat::get)
    }

    /// Float maximum bound, when declared.
    #[must_use]
    pub fn float_max(&self) -> Option<f64> {
        self.float_max.map(FiniteFloat::get)
    }

    /// Whether [`FieldContract::float_min`] is exclusive.
    #[must_use]
    pub fn float_min_exclusive(&self) -> bool {
        self.float_min_exclusive
    }

    /// Whether [`FieldContract::float_max`] is exclusive.
    #[must_use]
    pub fn float_max_exclusive(&self) -> bool {
        self.float_max_exclusive
    }

    /// Attach a non-blank description used by discovery.
    ///
    /// # Errors
    ///
    /// Fails when `description` is empty or only whitespace.
    pub fn with_description(
        mut self,
        description: impl Into<String>,
    ) -> Result<Self, InvalidContract> {
        let description = description.into();
        if is_blank(&description) {
            return Err(InvalidContract::EmptyDescription);
        }
        self.description = description;
        Ok(self)
    }

    /// Attach a unit label. Units are discovery metadata, not a value check.
    ///
    /// # Errors
    ///
    /// Fails when `unit` is empty or only whitespace.
    pub fn with_unit(mut self, unit: impl Into<String>) -> Result<Self, InvalidContract> {
        let unit = unit.into();
        if is_blank(&unit) {
            return Err(InvalidContract::EmptyUnit);
        }
        self.unit = Some(unit);
        Ok(self)
    }

    /// Restrict a string field to `values`.
    ///
    /// # Errors
    ///
    /// Fails when the field is not a string, `values` is empty, a value is
    /// blank, or a value is repeated.
    pub fn with_enum_values(mut self, values: Vec<String>) -> Result<Self, InvalidContract> {
        if self.contract != TypeContract::String {
            return Err(InvalidContract::EnumOnNonString);
        }
        if values.is_empty() {
            return Err(InvalidContract::EmptyEnum);
        }
        let mut seen = std::collections::HashSet::new();
        for value in &values {
            if is_blank(value) {
                return Err(InvalidContract::EmptyEnumValue);
            }
            if !seen.insert(value.clone()) {
                return Err(InvalidContract::DuplicateEnumValue {
                    value: value.clone(),
                });
            }
        }
        self.enum_values = Some(values);
        Ok(self)
    }

    /// Inclusive integer range. Either bound may be omitted.
    ///
    /// # Errors
    ///
    /// Fails when the field is not an integer or `min` is greater than `max`.
    pub fn with_integer_range(
        mut self,
        min: Option<i64>,
        max: Option<i64>,
    ) -> Result<Self, InvalidContract> {
        if self.contract != TypeContract::Integer {
            return Err(InvalidContract::IntegerRangeOnNonInteger);
        }
        if let (Some(min), Some(max)) = (min, max)
            && min > max
        {
            return Err(InvalidContract::InvalidIntegerRange);
        }
        self.integer_min = min;
        self.integer_max = max;
        Ok(self)
    }

    /// Float range. Exclusive flags apply only when the corresponding bound is
    /// present.
    ///
    /// # Errors
    ///
    /// Fails when the field is not a float, a bound is non-finite, or the
    /// minimum is greater than the maximum.
    pub fn with_float_range(mut self, range: FloatRange) -> Result<Self, InvalidContract> {
        if self.contract != TypeContract::Float {
            return Err(InvalidContract::FloatRangeOnNonFloat);
        }
        let float_min = finite_bound(range.min)?;
        let float_max = finite_bound(range.max)?;
        if let (Some(low), Some(high)) = (float_min, float_max)
            && low.get() > high.get()
        {
            return Err(InvalidContract::InvalidFloatRange);
        }
        self.float_min = float_min;
        self.float_max = float_max;
        self.float_min_exclusive = range.min_exclusive && float_min.is_some();
        self.float_max_exclusive = range.max_exclusive && float_max.is_some();
        Ok(self)
    }
}

fn is_blank(value: &str) -> bool {
    value.is_empty() || value.chars().all(char::is_whitespace)
}

fn finite_bound(value: Option<f64>) -> Result<Option<FiniteFloat>, InvalidContract> {
    match value {
        None => Ok(None),
        Some(raw) => FiniteFloat::try_new(raw)
            .map(Some)
            .map_err(|_| InvalidContract::NonFiniteFloatBound),
    }
}

/// Expected shape of a [`crate::Value`].
///
/// Contracts are data, not Rust `TypeId`s. Object field order is the order
/// supplied to [`TypeContract::object`]. Objects are strict: undeclared fields
/// are rejected during validation.
///
/// Composite contracts are built through [`TypeContract::object`] and
/// [`TypeContract::list`]. List and object payloads are crate-private so
/// callers cannot insert empty or duplicate field names by constructing or
/// mutably matching variants. Observe shapes with [`TypeContract::fields`],
/// [`TypeContract::item`], and [`TypeContract::value_kind`].
///
/// ```compile_fail
/// use axiom_rs::{FieldContract, ObjectContract, TypeContract};
/// let _ = TypeContract::Object(ObjectContract {
///     fields: vec![FieldContract::new("", TypeContract::Null, true)],
/// });
/// ```
///
/// ```compile_fail
/// use axiom_rs::{FieldContract, ObjectContract, TypeContract};
/// let mut contract = TypeContract::object(vec![
///     FieldContract::new("a", TypeContract::Null, true),
/// ])
/// .unwrap();
/// if let TypeContract::Object(object) = &mut contract {
///     object.fields.push(FieldContract::new("", TypeContract::Null, true));
/// }
/// ```
///
/// ```compile_fail
/// use axiom_rs::{FieldContract, ListContract, TypeContract};
/// let mut contract = TypeContract::list(TypeContract::object(vec![]).unwrap());
/// if let TypeContract::List(list) = &mut contract {
///     list.item = Box::new(TypeContract::Object(axiom_rs::ObjectContract {
///         fields: vec![FieldContract::new("", TypeContract::Null, true)],
///     }));
/// }
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
    /// Homogeneous list. Inspect with [`TypeContract::item`].
    List(ListContract),
    /// Strict object. Created only by [`TypeContract::object`].
    Object(ObjectContract),
}

/// Item contract of a list. Fields are private so callers cannot swap in an
/// unvalidated nested object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListContract {
    pub(crate) item: Box<TypeContract>,
}

/// Field list of an object contract. Fields are private so callers cannot
/// insert empty or duplicate names after construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectContract {
    pub(crate) fields: Vec<FieldContract>,
}

impl TypeContract {
    /// List whose items match `item`.
    #[must_use]
    pub fn list(item: TypeContract) -> Self {
        Self::List(ListContract {
            item: Box::new(item),
        })
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
        Ok(Self::Object(ObjectContract { fields }))
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
            Self::List(_) => ValueKind::List,
            Self::Object(_) => ValueKind::Object,
        }
    }

    /// Object fields in declaration order.
    #[must_use]
    pub fn fields(&self) -> Option<&[FieldContract]> {
        match self {
            Self::Object(object) => Some(&object.fields),
            _ => None,
        }
    }

    /// List item contract.
    #[must_use]
    pub fn item(&self) -> Option<&TypeContract> {
        match self {
            Self::List(list) => Some(&list.item),
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

    #[test]
    fn crate_internal_object_fields_stay_valid_after_read() {
        let contract =
            TypeContract::object(vec![FieldContract::new("a", TypeContract::Null, true)]).unwrap();
        if let TypeContract::Object(object) = &contract {
            assert_eq!(object.fields[0].name(), "a");
        } else {
            panic!("expected object");
        }
        assert_eq!(contract.fields().unwrap()[0].name(), "a");
    }

    #[test]
    fn constraint_constructors_reject_invalid_declarations() {
        use super::FloatRange;
        assert_eq!(
            FieldContract::new("u", TypeContract::Integer, true)
                .with_enum_values(vec!["a".into()])
                .unwrap_err(),
            InvalidContract::EnumOnNonString
        );
        assert_eq!(
            FieldContract::new("n", TypeContract::Integer, true)
                .with_integer_range(Some(3), Some(1))
                .unwrap_err(),
            InvalidContract::InvalidIntegerRange
        );
        assert_eq!(
            FieldContract::new("x", TypeContract::Float, true)
                .with_float_range(FloatRange {
                    min: Some(f64::NAN),
                    max: None,
                    min_exclusive: false,
                    max_exclusive: false,
                })
                .unwrap_err(),
            InvalidContract::NonFiniteFloatBound
        );
        assert!(
            FieldContract::new("d", TypeContract::String, true)
                .with_description("ok")
                .unwrap()
                .description()
                .is_some()
        );
        assert_eq!(
            FieldContract::new("d", TypeContract::String, true)
                .with_description("   ")
                .unwrap_err(),
            InvalidContract::EmptyDescription
        );
        assert_eq!(
            FieldContract::new("u", TypeContract::String, true)
                .with_unit(" ")
                .unwrap_err(),
            InvalidContract::EmptyUnit
        );
    }

    #[test]
    fn constraint_accessors_preserve_declared_bounds() {
        use super::FloatRange;
        let unit = FieldContract::new("u", TypeContract::String, true)
            .with_unit("mm")
            .unwrap();
        assert_eq!(unit.unit(), Some("mm"));
        assert!(
            FieldContract::new("u", TypeContract::String, true)
                .unit()
                .is_none()
        );
        let int = FieldContract::new("n", TypeContract::Integer, true)
            .with_integer_range(Some(0), Some(0))
            .unwrap();
        assert_eq!(int.integer_min(), Some(0));
        assert_eq!(int.integer_max(), Some(0));
        let float = FieldContract::new("x", TypeContract::Float, true)
            .with_float_range(FloatRange {
                min: Some(1.0),
                max: Some(1.0),
                min_exclusive: false,
                max_exclusive: true,
            })
            .unwrap();
        assert_eq!(float.float_min(), Some(1.0));
        assert_eq!(float.float_max(), Some(1.0));
        assert!(!float.float_min_exclusive());
        assert!(float.float_max_exclusive());
        let unbounded = FieldContract::new("x", TypeContract::Float, true)
            .with_float_range(FloatRange {
                min: None,
                max: None,
                min_exclusive: true,
                max_exclusive: true,
            })
            .unwrap();
        assert!(unbounded.float_min().is_none());
        assert!(unbounded.float_max().is_none());
        assert!(!unbounded.float_min_exclusive());
        assert!(!unbounded.float_max_exclusive());
        assert!(
            FieldContract::new("n", TypeContract::Integer, true)
                .integer_min()
                .is_none()
        );
    }
}
