//! Protocol-independent dynamic values.

use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Classification of a [`Value`] that does not inspect nested content.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ValueKind {
    /// JSON-like null.
    Null,
    /// Boolean.
    Bool,
    /// 64-bit signed integer.
    Integer,
    /// Finite floating-point number.
    Float,
    /// UTF-8 string.
    String,
    /// Ordered list.
    List,
    /// String-keyed object.
    Object,
}

impl ValueKind {
    /// Stable name used in error details and documentation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool => "bool",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::String => "string",
            Self::List => "list",
            Self::Object => "object",
        }
    }
}

impl fmt::Display for ValueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Finite `f64` stored inside [`Value`].
#[derive(Clone, Copy, Debug)]
pub struct FiniteFloat(f64);

impl FiniteFloat {
    fn bits(self) -> u64 {
        if self.0 == 0.0 {
            0.0f64.to_bits()
        } else {
            self.0.to_bits()
        }
    }

    /// The finite floating-point value.
    #[must_use]
    pub fn get(self) -> f64 {
        self.0
    }
}

impl PartialEq for FiniteFloat {
    fn eq(&self, other: &Self) -> bool {
        self.bits() == other.bits()
    }
}

impl Eq for FiniteFloat {}

impl Hash for FiniteFloat {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bits().hash(state);
    }
}

/// Dynamic value used at runtime boundaries.
///
/// Integers are `i64`. Floats are finite `f64` values; NaN and infinities are
/// rejected. Construction never silently truncates or parses strings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    /// Null scalar.
    Null,
    /// Boolean scalar.
    Bool(bool),
    /// Integer scalar.
    Integer(i64),
    /// Finite float scalar.
    Float(FiniteFloat),
    /// String scalar.
    String(String),
    /// Nested list.
    List(Vec<Value>),
    /// Nested object.
    Object(Object),
}

/// Ordered object with unique string keys.
///
/// Iteration follows insertion order. Equality and hashing ignore insertion
/// order and compare keys with their values.
#[derive(Clone, Debug)]
pub struct Object {
    fields: Vec<(String, Value)>,
}

/// Duplicate key encountered while building an object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DuplicateField {
    /// Repeated field name.
    pub name: String,
}

/// Non-finite floating point value rejected by [`Value::try_float`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NonFiniteFloat {
    bits: u64,
}

impl NonFiniteFloat {
    /// IEEE-754 bits of the rejected value.
    #[must_use]
    pub fn to_bits(self) -> u64 {
        self.bits
    }
}

impl Value {
    /// Null value.
    #[must_use]
    pub fn null() -> Self {
        Self::Null
    }

    /// Boolean value.
    #[must_use]
    pub fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    /// Integer value.
    #[must_use]
    pub fn integer(value: i64) -> Self {
        Self::Integer(value)
    }

    /// Finite float value.
    ///
    /// # Errors
    ///
    /// Returns [`NonFiniteFloat`] when `value` is NaN or infinite.
    pub fn try_float(value: f64) -> Result<Self, NonFiniteFloat> {
        if value.is_finite() {
            Ok(Self::Float(FiniteFloat(value)))
        } else {
            Err(NonFiniteFloat {
                bits: value.to_bits(),
            })
        }
    }

    /// String value.
    #[must_use]
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    /// List value.
    #[must_use]
    pub fn list(values: impl IntoIterator<Item = Value>) -> Self {
        Self::List(values.into_iter().collect())
    }

    /// Object value with unique keys preserved in insertion order.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateField`] when a key appears more than once.
    pub fn try_object<K, I>(fields: I) -> Result<Self, DuplicateField>
    where
        K: Into<String>,
        I: IntoIterator<Item = (K, Value)>,
    {
        Ok(Self::Object(Object::try_from_entries(fields)?))
    }

    /// Observable kind of this value.
    #[must_use]
    pub fn kind(&self) -> ValueKind {
        match self {
            Self::Null => ValueKind::Null,
            Self::Bool(_) => ValueKind::Bool,
            Self::Integer(_) => ValueKind::Integer,
            Self::Float(_) => ValueKind::Float,
            Self::String(_) => ValueKind::String,
            Self::List(_) => ValueKind::List,
            Self::Object(_) => ValueKind::Object,
        }
    }

    /// Borrow a boolean without conversion.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// Borrow an integer without conversion.
    #[must_use]
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    /// Borrow a finite float without conversion.
    #[must_use]
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(value.0),
            _ => None,
        }
    }

    /// Borrow a string without conversion.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    /// Borrow list items without conversion.
    #[must_use]
    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Self::List(values) => Some(values),
            _ => None,
        }
    }

    /// Borrow an object without conversion.
    #[must_use]
    pub fn as_object(&self) -> Option<&Object> {
        match self {
            Self::Object(object) => Some(object),
            _ => None,
        }
    }

    /// Take list ownership without conversion.
    pub fn into_list(self) -> Result<Vec<Value>, Self> {
        match self {
            Self::List(values) => Ok(values),
            other => Err(other),
        }
    }

    /// Take object ownership without conversion.
    pub fn into_object(self) -> Result<Object, Self> {
        match self {
            Self::Object(object) => Ok(object),
            other => Err(other),
        }
    }
}

impl Object {
    /// Build an object from `fields`.
    ///
    /// # Errors
    ///
    /// Fails when a key is repeated. Existing entries are not overwritten.
    pub fn try_from_entries<K, I>(fields: I) -> Result<Self, DuplicateField>
    where
        K: Into<String>,
        I: IntoIterator<Item = (K, Value)>,
    {
        let mut seen = HashSet::new();
        let mut ordered = Vec::new();
        for (key, value) in fields {
            let name = key.into();
            if !seen.insert(name.clone()) {
                return Err(DuplicateField { name });
            }
            ordered.push((name, value));
        }
        Ok(Self { fields: ordered })
    }

    /// Number of fields.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Whether the object has no fields.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Look up `name` without mutating the object.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.fields
            .iter()
            .find_map(|(key, value)| (key == name).then_some(value))
    }

    /// Iterate fields in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.fields.iter().map(|(key, value)| (key.as_str(), value))
    }
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        if self.fields.len() != other.fields.len() {
            return false;
        }
        self.fields
            .iter()
            .all(|(key, value)| other.get(key) == Some(value))
    }
}

impl Eq for Object {}

impl Hash for Object {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut entries: Vec<_> = self
            .fields
            .iter()
            .map(|(key, value)| (key.as_str(), value))
            .collect();
        entries.sort_by_key(|(key, _)| *key);
        entries.hash(state);
    }
}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Null => {}
            Self::Bool(value) => value.hash(state),
            Self::Integer(value) => value.hash(state),
            Self::Float(value) => value.hash(state),
            Self::String(value) => value.hash(state),
            Self::List(values) => values.hash(state),
            Self::Object(object) => object.hash(state),
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::bool(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::integer(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::string(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::string(value)
    }
}

impl fmt::Display for DuplicateField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "duplicate object field {}", self.name)
    }
}

impl fmt::Display for NonFiniteFloat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("float must be finite")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_of<T: Hash>(value: &T) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn scalar_kinds() {
        assert_eq!(Value::null().kind(), ValueKind::Null);
        assert_eq!(Value::bool(true).kind(), ValueKind::Bool);
        assert_eq!(Value::integer(1).kind(), ValueKind::Integer);
        assert_eq!(Value::try_float(1.5).unwrap().kind(), ValueKind::Float);
        assert_eq!(Value::string("x").kind(), ValueKind::String);
    }

    #[test]
    fn integer_boundaries() {
        assert_eq!(Value::integer(i64::MIN).as_integer(), Some(i64::MIN));
        assert_eq!(Value::integer(i64::MAX).as_integer(), Some(i64::MAX));
        assert_eq!(Value::bool(true).as_bool(), Some(true));
        assert_eq!(Value::bool(false).as_bool(), Some(false));
        assert_eq!(Value::integer(1).as_bool(), None);
        assert_eq!(Value::integer(1).as_float(), None);
        assert_eq!(Value::integer(1).as_str(), None);
    }

    #[test]
    fn rejects_non_finite_floats() {
        let nan = Value::try_float(f64::NAN).unwrap_err();
        assert_eq!(nan.to_bits(), f64::NAN.to_bits());
        assert!(Value::try_float(f64::INFINITY).is_err());
        assert!(Value::try_float(f64::NEG_INFINITY).is_err());
        assert_eq!(Value::try_float(1.25).unwrap().as_float(), Some(1.25));
        let zero = Value::try_float(0.0).unwrap();
        let neg_zero = Value::try_float(-0.0).unwrap();
        assert_eq!(zero, neg_zero);
        assert_eq!(hash_of(&zero), hash_of(&neg_zero));
        assert_ne!(
            Value::try_float(1.25).unwrap(),
            Value::try_float(2.5).unwrap()
        );
    }

    #[test]
    fn no_silent_conversions() {
        assert_eq!(Value::from("1").as_integer(), None);
        assert_eq!(Value::from(true).as_integer(), None);
        assert_eq!(Value::try_float(1.0).unwrap().as_integer(), None);
        assert_eq!(Value::integer(1).as_str(), None);
    }

    #[test]
    fn scalar_display_and_float_access() {
        assert_eq!(ValueKind::Integer.to_string(), "integer");
        assert_eq!(
            DuplicateField { name: "a".into() }.to_string(),
            "duplicate object field a"
        );
        if let Value::Float(finite) = Value::try_float(2.5).unwrap() {
            assert_eq!(finite.get(), 2.5);
        } else {
            panic!("expected float");
        }
    }

    #[test]
    fn nested_list_and_object() {
        let value =
            Value::try_object([("items", Value::list([Value::integer(1), Value::from("x")]))])
                .unwrap();
        let object = value.as_object().unwrap();
        let items = object.get("items").unwrap().as_list().unwrap();
        assert_eq!(items[0], Value::integer(1));
        assert_eq!(items[1], Value::from("x"));
    }

    #[test]
    fn empty_collections() {
        let list = Value::list([]);
        assert_eq!(list.as_list().unwrap().len(), 0);
        let object = Value::try_object::<&str, _>([]).unwrap();
        assert!(object.as_object().unwrap().is_empty());
        assert_eq!(object.as_object().unwrap().len(), 0);
    }

    #[test]
    fn duplicate_keys_are_rejected() {
        let err =
            Object::try_from_entries([("a", Value::null()), ("a", Value::bool(true))]).unwrap_err();
        assert_eq!(err.name, "a");
    }

    #[test]
    fn lookup_and_iter_do_not_mutate() {
        let object =
            Object::try_from_entries([("b", Value::integer(2)), ("a", Value::integer(1))]).unwrap();
        let before = object.iter().map(|(k, _)| k.to_owned()).collect::<Vec<_>>();
        assert_eq!(object.get("a"), Some(&Value::integer(1)));
        let after = object.iter().map(|(k, _)| k.to_owned()).collect::<Vec<_>>();
        assert_eq!(before, after);
        assert_eq!(before, ["b", "a"]);
    }

    #[test]
    fn object_equality_ignores_insertion_order() {
        let left =
            Object::try_from_entries([("a", Value::integer(1)), ("b", Value::integer(2))]).unwrap();
        let right =
            Object::try_from_entries([("b", Value::integer(2)), ("a", Value::integer(1))]).unwrap();
        assert_eq!(left, right);
        assert_eq!(hash_of(&left), hash_of(&right));
        assert_ne!(
            left,
            Object::try_from_entries([("a", Value::integer(1))]).unwrap()
        );
    }

    #[test]
    fn ownership_conversion() {
        let list = Value::list([Value::null()]);
        assert_eq!(list.clone().into_list().unwrap().len(), 1);
        assert!(list.into_object().is_err());
        let object = Value::try_object([("k", Value::null())]).unwrap();
        assert!(!object.as_object().unwrap().is_empty());
        assert_eq!(object.clone().into_object().unwrap().len(), 1);
        assert!(object.into_list().is_err());
    }
}
