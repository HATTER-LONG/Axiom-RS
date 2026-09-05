//! Command response encoding.

use crate::execution::ExecutionContext;
use crate::foundation::{Error, Value};

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
    let path = match error.path() {
        Some(path) if !path.segments().is_empty() => Value::string(path.to_string()),
        _ => Value::null(),
    };
    let details = error.details().cloned().unwrap_or(Value::null());
    Value::try_object([
        ("kind", Value::string(error.kind().as_str())),
        ("message", Value::string(error.message())),
        ("path", path),
        ("details", details),
    ])
    .expect("error keys are unique")
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
        assert_eq!(err.get("path").unwrap().as_str(), Some("n"));
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
    fn root_path_encodes_as_null() {
        let error = crate::foundation::Error::type_mismatch(
            Path::root(),
            crate::foundation::ValueKind::Integer,
            crate::foundation::ValueKind::String,
        );
        let err = CommandResponse::err(error)
            .to_value()
            .as_object()
            .unwrap()
            .get("error")
            .unwrap()
            .as_object()
            .unwrap()
            .get("path")
            .cloned();
        assert_eq!(err, Some(Value::null()));
        assert!(!CommandResponse::err(crate::foundation::Error::unknown_command("x")).is_ok());
    }
}
