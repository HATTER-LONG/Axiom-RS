//! Command response encoding.
//!
//! Error `path` uses a structured object when a path is present:
//! `{ "segments": [...], "display": "..." }`. `segments` is empty for
//! [`crate::Path::root`]. `path` is JSON `null` when the error has no path.
//!
//! Path bases:
//! - command envelope failures are relative to the request object;
//! - invoke input contract failures are relative to the capability `input`
//!   value (not prefixed with `input`);
//! - output contract violations are relative to the capability output value.

use crate::execution::ExecutionContext;
use crate::foundation::{Error, Path, PathSegment, Value};

use super::decode::COMMAND_VERSION;

/// Unified command response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandResponse {
    result: Result<Value, Error>,
    correlation_id: Option<String>,
    parent_id: Option<String>,
}

impl CommandResponse {
    pub(crate) fn ok(value: Value) -> Self {
        Self {
            result: Ok(value),
            correlation_id: None,
            parent_id: None,
        }
    }

    pub(crate) fn err(error: Error) -> Self {
        Self {
            result: Err(error),
            correlation_id: None,
            parent_id: None,
        }
    }

    /// Wrap a structured error as a command response.
    #[must_use]
    pub fn from_error(error: Error) -> Self {
        Self::err(error)
    }

    pub(crate) fn with_context(mut self, context: &ExecutionContext) -> Self {
        self.correlation_id = Some(context.correlation_id().as_str().to_owned());
        self.parent_id = context.parent_id().map(|id| id.as_str().to_owned());
        self
    }

    /// Whether the command succeeded.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.result.is_ok()
    }

    /// Success payload.
    #[must_use]
    pub fn value(&self) -> Option<&Value> {
        self.result.as_ref().ok()
    }

    /// Failure payload.
    #[must_use]
    pub fn error(&self) -> Option<&Error> {
        self.result.as_ref().err()
    }

    /// Correlation identifier copied from an invoke request, if any.
    #[must_use]
    pub fn correlation_id(&self) -> Option<&str> {
        self.correlation_id.as_deref()
    }

    /// Parent identifier copied from an invoke request, if any.
    #[must_use]
    pub fn parent_id(&self) -> Option<&str> {
        self.parent_id.as_deref()
    }

    /// Encode the response envelope as a [`Value`].
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut fields = vec![
            ("v", Value::integer(COMMAND_VERSION)),
            ("ok", Value::bool(self.result.is_ok())),
        ];
        match &self.result {
            Ok(value) => fields.push(("value", value.clone())),
            Err(error) => fields.push(("error", error_to_value(error))),
        }
        if let Some(id) = &self.correlation_id {
            fields.push(("correlation_id", Value::string(id.clone())));
        }
        if let Some(id) = &self.parent_id {
            fields.push(("parent_id", Value::string(id.clone())));
        }
        Value::try_object(fields).expect("response keys are unique")
    }
}

pub(crate) fn error_to_value(error: &Error) -> Value {
    let details = error.details().cloned().unwrap_or(Value::null());
    Value::try_object([
        ("kind", Value::string(error.kind().as_str())),
        ("message", Value::string(error.message())),
        ("path", path_to_value(error.path())),
        ("details", details),
    ])
    .expect("error keys are unique")
}

fn path_to_value(path: Option<&Path>) -> Value {
    match path {
        None => Value::null(),
        Some(path) => Value::try_object([
            (
                "segments",
                Value::list(path.segments().iter().map(segment_to_value)),
            ),
            ("display", Value::string(path.to_string())),
        ])
        .expect("path keys are unique"),
    }
}

fn segment_to_value(segment: &PathSegment) -> Value {
    match segment {
        PathSegment::Field(name) => Value::try_object([
            ("kind", Value::string("field")),
            ("name", Value::string(name.clone())),
        ])
        .expect("segment keys are unique"),
        PathSegment::Index(index) => Value::try_object([
            ("kind", Value::string("index")),
            (
                "index",
                Value::integer(i64::try_from(*index).expect("path index fits i64")),
            ),
        ])
        .expect("segment keys are unique"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::{ErrorKind, Path};

    #[test]
    fn encodes_error_kind_and_path() {
        let error = crate::foundation::Error::type_mismatch(
            Path::root().field("n"),
            crate::foundation::ValueKind::Integer,
            crate::foundation::ValueKind::String,
        );
        let encoded = CommandResponse::err(error).to_value();
        let object = encoded.as_object().unwrap();
        assert_eq!(object.get("ok").unwrap().as_bool(), Some(false));
        let err = object.get("error").unwrap().as_object().unwrap();
        assert_eq!(
            err.get("kind").unwrap().as_str(),
            Some(ErrorKind::TypeMismatch.as_str())
        );
        let path = err.get("path").unwrap().as_object().unwrap();
        assert_eq!(path.get("display").and_then(Value::as_str), Some("n"));
        let segments = path.get("segments").unwrap().as_list().unwrap();
        assert_eq!(
            segments[0].as_object().unwrap().get("name"),
            Some(&Value::string("n"))
        );
    }

    #[test]
    fn success_and_parent_context_are_observable() {
        let ok = CommandResponse::ok(Value::integer(1));
        assert!(ok.is_ok());
        assert!(ok.parent_id().is_none());
        let ctx = crate::execution::ExecutionContext::root(
            crate::foundation::CorrelationId::parse("p").unwrap(),
        )
        .child(crate::foundation::CorrelationId::parse("c").unwrap());
        let with_parent = CommandResponse::ok(Value::null()).with_context(&ctx);
        assert_eq!(with_parent.parent_id(), Some("p"));
        assert_eq!(
            with_parent
                .to_value()
                .as_object()
                .unwrap()
                .get("parent_id")
                .and_then(Value::as_str),
            Some("p")
        );
    }

    #[test]
    fn root_path_is_not_null() {
        let error = crate::foundation::Error::type_mismatch(
            Path::root(),
            crate::foundation::ValueKind::Integer,
            crate::foundation::ValueKind::String,
        );
        let path = CommandResponse::err(error)
            .to_value()
            .as_object()
            .unwrap()
            .get("error")
            .unwrap()
            .as_object()
            .unwrap()
            .get("path")
            .cloned();
        let object = path.unwrap().as_object().unwrap().clone();
        assert_eq!(
            object
                .get("segments")
                .and_then(Value::as_list)
                .map(|segments| segments.len()),
            Some(0)
        );
        assert_eq!(object.get("display").and_then(Value::as_str), Some(""));
        let missing = CommandResponse::err(crate::foundation::Error::unknown_command("x"));
        assert!(!missing.is_ok());
        let missing_value = missing.to_value();
        let missing_path = missing_value
            .as_object()
            .unwrap()
            .get("error")
            .unwrap()
            .as_object()
            .unwrap()
            .get("path");
        assert!(missing_path.unwrap().as_object().is_some());
        let none_path = crate::foundation::Error::unknown_capability("gone");
        assert!(none_path.path().is_none());
        assert_eq!(
            CommandResponse::err(none_path)
                .to_value()
                .as_object()
                .unwrap()
                .get("error")
                .unwrap()
                .as_object()
                .unwrap()
                .get("path"),
            Some(&Value::null())
        );
    }

    #[test]
    fn index_and_special_field_segments() {
        let error = crate::foundation::Error::type_mismatch(
            Path::root().field("$i").index(2).field("has.dot"),
            crate::foundation::ValueKind::Integer,
            crate::foundation::ValueKind::String,
        );
        let encoded = CommandResponse::err(error).to_value();
        let path = encoded
            .as_object()
            .unwrap()
            .get("error")
            .unwrap()
            .as_object()
            .unwrap()
            .get("path")
            .unwrap()
            .as_object()
            .unwrap();
        let segments = path.get("segments").unwrap().as_list().unwrap();
        assert_eq!(segments.len(), 3);
        assert_eq!(
            segments[0].as_object().unwrap().get("kind"),
            Some(&Value::string("field"))
        );
        assert_eq!(
            segments[1].as_object().unwrap().get("index"),
            Some(&Value::integer(2))
        );
        assert_eq!(
            path.get("display").and_then(Value::as_str),
            Some("[\"$i\"][2][\"has.dot\"]")
        );
    }
}
