//! Structured errors for programs, tests, and agents.

use std::error::Error as StdError;
use std::fmt;

use super::id::InvalidIdentifier;
use super::path::Path;
use super::value::{Value, ValueKind};

/// Stable error category that can be matched without parsing [`Error`] display text.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ErrorKind {
    /// An identifier failed validation.
    InvalidIdentifier,
    /// A required object field was absent.
    MissingField,
    /// An object field was not declared by the contract.
    UnknownField,
    /// A value kind did not match the expected contract.
    TypeMismatch,
    /// A value was the expected kind but semantically invalid.
    InvalidValue,
}

impl ErrorKind {
    /// Stable machine-readable kind name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidIdentifier => "invalid_identifier",
            Self::MissingField => "missing_field",
            Self::UnknownField => "unknown_field",
            Self::TypeMismatch => "type_mismatch",
            Self::InvalidValue => "invalid_value",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Program-readable failure with a kind, optional path, and Axiom [`Value`] details.
///
/// Display text is for humans. Callers must use [`Error::kind`] to distinguish
/// categories. Only the module that owns a failure rule constructs its errors,
/// which keeps kind, path, and details aligned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    kind: ErrorKind,
    message: String,
    path: Option<Path>,
    details: Option<Value>,
}

impl Error {
    /// Identifier construction failure.
    #[must_use]
    pub(crate) fn invalid_identifier(cause: InvalidIdentifier) -> Self {
        let reason = match &cause {
            InvalidIdentifier::Empty => "empty",
            InvalidIdentifier::TooLong { .. } => "too_long",
            InvalidIdentifier::InvalidCharacter { .. } => "invalid_character",
        };
        Self {
            kind: ErrorKind::InvalidIdentifier,
            message: cause.to_string(),
            path: None,
            details: Some(detail_fields([("reason", Value::string(reason))])),
        }
    }

    /// Required object field was absent. `path` locates the missing field.
    #[must_use]
    pub(crate) fn missing_field(path: Path, field: impl Into<String>) -> Self {
        let field = field.into();
        Self {
            kind: ErrorKind::MissingField,
            message: format!("missing field {field}"),
            path: Some(path),
            details: Some(detail_fields([("field", Value::string(field))])),
        }
    }

    /// Object contained a field not declared by the contract.
    #[must_use]
    pub(crate) fn unknown_field(path: Path, field: impl Into<String>) -> Self {
        let field = field.into();
        Self {
            kind: ErrorKind::UnknownField,
            message: format!("unknown field {field}"),
            path: Some(path),
            details: Some(detail_fields([("field", Value::string(field))])),
        }
    }

    /// Value kind did not match the contract at `path`.
    #[must_use]
    pub(crate) fn type_mismatch(path: Path, expected: ValueKind, actual: ValueKind) -> Self {
        Self {
            kind: ErrorKind::TypeMismatch,
            message: format!("expected {expected}, found {actual}"),
            path: Some(path),
            details: Some(detail_fields([
                ("expected", Value::string(expected.as_str())),
                ("actual", Value::string(actual.as_str())),
            ])),
        }
    }

    /// Error category.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Human-readable message. Not the machine-readable kind.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Diagnostic path, when the error refers to nested input.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_ref()
    }

    /// Structured details encoded as [`Value`].
    #[must_use]
    pub fn details(&self) -> Option<&Value> {
        self.details.as_ref()
    }
}

fn detail_fields<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::try_object(fields).expect("detail keys are unique")
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(path) if !path.segments().is_empty() => {
                write!(f, "{} at {}: {}", self.kind, path, self.message)
            }
            _ => write!(f, "{}: {}", self.kind, self.message),
        }
    }
}

impl StdError for Error {}

impl From<InvalidIdentifier> for Error {
    fn from(value: InvalidIdentifier) -> Self {
        Self::invalid_identifier(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::CorrelationId;

    #[test]
    fn kinds_are_distinct_without_parsing_display() {
        let missing = Error::missing_field(Path::root().field("a"), "a");
        let unknown = Error::unknown_field(Path::root().field("b"), "b");
        let mismatch = Error::type_mismatch(Path::root(), ValueKind::Integer, ValueKind::String);
        assert_eq!(missing.kind(), ErrorKind::MissingField);
        assert_eq!(unknown.kind(), ErrorKind::UnknownField);
        assert_eq!(mismatch.kind(), ErrorKind::TypeMismatch);
        assert_ne!(missing.kind(), unknown.kind());
    }

    #[test]
    fn nested_path_is_preserved() {
        let path = Path::root().field("options").index(1).field("size");
        let error = Error::type_mismatch(path.clone(), ValueKind::Integer, ValueKind::Bool);
        assert_eq!(error.path(), Some(&path));
        assert!(error.to_string().contains("options[1].size"));
    }

    #[test]
    fn details_are_axiom_values() {
        let error = Error::type_mismatch(
            Path::root().field("n"),
            ValueKind::Integer,
            ValueKind::String,
        );
        let details = error.details().unwrap().as_object().unwrap();
        assert_eq!(
            details.get("expected").and_then(Value::as_str),
            Some("integer")
        );
        assert_eq!(
            details.get("actual").and_then(Value::as_str),
            Some("string")
        );
    }

    #[test]
    fn identifier_error_round_trip() {
        let cause = CorrelationId::parse("").unwrap_err();
        let error = Error::from(cause);
        assert_eq!(error.kind(), ErrorKind::InvalidIdentifier);
        assert!(error.path().is_none());
        assert_eq!(
            error.details().unwrap().as_object().unwrap().get("reason"),
            Some(&Value::string("empty"))
        );
    }

    #[test]
    fn display_is_not_the_kind() {
        let error = Error::unknown_field(Path::root().field("x"), "x");
        assert!(error.to_string().contains("unknown_field"));
        assert_eq!(error.kind(), ErrorKind::UnknownField);
        assert_eq!(ErrorKind::MissingField.as_str(), "missing_field");
        assert_eq!(error.message(), "unknown field x");
    }

    #[test]
    fn root_path_is_omitted_from_display() {
        let error = Error::type_mismatch(Path::root(), ValueKind::Integer, ValueKind::String);
        assert_eq!(
            error.to_string(),
            "type_mismatch: expected integer, found string"
        );
    }

    #[test]
    fn missing_and_unknown_include_field_detail() {
        let missing = Error::missing_field(Path::root().field("need"), "need");
        let unknown = Error::unknown_field(Path::root().field("extra"), "extra");
        assert_eq!(
            missing.details().unwrap().as_object().unwrap().get("field"),
            Some(&Value::from("need"))
        );
        assert_eq!(
            unknown.details().unwrap().as_object().unwrap().get("field"),
            Some(&Value::from("extra"))
        );
    }
}
