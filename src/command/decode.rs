//! Strict command envelope decoding.

use crate::capability::CapabilityName;
use crate::execution::ExecutionContext;
use crate::foundation::{CorrelationId, Error, Object, Path, Value};

use super::request_error;

/// Supported request version.
pub const COMMAND_VERSION: i64 = 1;

/// Protocol-independent command after envelope validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    /// List registered capabilities.
    List,
    /// Fetch one descriptor by name.
    Get {
        /// Capability name.
        name: CapabilityName,
    },
    /// Invoke a capability.
    Invoke {
        /// Capability name.
        name: CapabilityName,
        /// Owned input value.
        input: Value,
        /// Correlation context from the request.
        context: ExecutionContext,
    },
}

const KNOWN_FIELDS: &[&str] = &["v", "cmd", "name", "input", "correlation_id", "parent_id"];

/// Decode a command object.
///
/// # Errors
///
/// Missing, extra, or mistyped envelope fields produce
/// [`crate::ErrorKind::InvalidRequest`] or [`crate::ErrorKind::UnknownCommand`].
pub fn decode(value: &Value) -> Result<Command, Error> {
    let object = value
        .as_object()
        .ok_or_else(|| request_error("v", "command request must be an object", Value::null()))?;
    reject_unknown(object)?;
    let version = require_integer(object, "v")?;
    if version != COMMAND_VERSION {
        return Err(request_error(
            "v",
            "unsupported command version",
            Value::integer(version),
        ));
    }
    let cmd = require_str(object, "cmd")?;
    match cmd {
        "list" => decode_list(object),
        "get" => decode_get(object),
        "invoke" => decode_invoke(object),
        other => Err(Error::unknown_command(other)),
    }
}

fn decode_list(object: &Object) -> Result<Command, Error> {
    reject_fields(object, &["name", "input", "correlation_id", "parent_id"])?;
    Ok(Command::List)
}

fn decode_get(object: &Object) -> Result<Command, Error> {
    reject_fields(object, &["input", "correlation_id", "parent_id"])?;
    Ok(Command::Get {
        name: parse_name(object)?,
    })
}

fn decode_invoke(object: &Object) -> Result<Command, Error> {
    let name = parse_name(object)?;
    let input = object
        .get("input")
        .cloned()
        .ok_or_else(|| request_error("input", "invoke requires input", Value::string("missing")))?;
    let correlation = parse_id(object, "correlation_id")?;
    let context = match object.get("parent_id") {
        None => ExecutionContext::root(correlation),
        Some(_) => {
            let parent = parse_id(object, "parent_id")?;
            ExecutionContext::root(parent).child(correlation)
        }
    };
    Ok(Command::Invoke {
        name,
        input,
        context,
    })
}

fn reject_unknown(object: &Object) -> Result<(), Error> {
    for (key, _) in object.iter() {
        if !KNOWN_FIELDS.contains(&key) {
            return Err(Error::invalid_request(
                Path::root().field(key),
                "unknown command field",
                Value::try_object([("field", Value::string(key))]).expect("unique"),
            ));
        }
    }
    Ok(())
}

fn reject_fields(object: &Object, fields: &[&str]) -> Result<(), Error> {
    for field in fields {
        if object.get(field).is_some() {
            return Err(request_error(
                field,
                "field is not allowed for this command",
                Value::string(*field),
            ));
        }
    }
    Ok(())
}

fn require_integer(object: &Object, field: &str) -> Result<i64, Error> {
    let value = object
        .get(field)
        .ok_or_else(|| request_error(field, "missing command field", Value::string(field)))?;
    value.as_integer().ok_or_else(|| {
        request_error(
            field,
            "command field must be an integer",
            Value::string(field),
        )
    })
}

fn require_str<'a>(object: &'a Object, field: &str) -> Result<&'a str, Error> {
    let value = object
        .get(field)
        .ok_or_else(|| request_error(field, "missing command field", Value::string(field)))?;
    value.as_str().ok_or_else(|| {
        request_error(
            field,
            "command field must be a string",
            Value::string(field),
        )
    })
}

fn parse_name(object: &Object) -> Result<CapabilityName, Error> {
    let raw = require_str(object, "name")?;
    CapabilityName::parse(raw)
        .map_err(|err| request_error("name", &err.to_string(), Value::string(raw)))
}

fn parse_id(object: &Object, field: &str) -> Result<CorrelationId, Error> {
    let raw = require_str(object, field)?;
    CorrelationId::parse(raw)
        .map_err(|err| request_error(field, &err.to_string(), Value::string(raw)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::ErrorKind;

    fn obj(fields: &[(&str, Value)]) -> Value {
        Value::try_object(fields.iter().cloned()).unwrap()
    }

    #[test]
    fn list_and_unknown_command() {
        let list = decode(&obj(&[
            ("v", Value::integer(1)),
            ("cmd", Value::string("list")),
        ]))
        .unwrap();
        assert_eq!(list, Command::List);
        let err = decode(&obj(&[
            ("v", Value::integer(1)),
            ("cmd", Value::string("drop")),
        ]))
        .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownCommand);
    }

    #[test]
    fn extra_and_missing_fields() {
        let extra = decode(&obj(&[
            ("v", Value::integer(1)),
            ("cmd", Value::string("list")),
            ("name", Value::string("x")),
        ]))
        .unwrap_err();
        assert_eq!(extra.kind(), ErrorKind::InvalidRequest);
        let missing = decode(&obj(&[("cmd", Value::string("list"))])).unwrap_err();
        assert_eq!(missing.kind(), ErrorKind::InvalidRequest);
        let unknown = decode(&obj(&[
            ("v", Value::integer(1)),
            ("cmd", Value::string("list")),
            ("nope", Value::null()),
        ]))
        .unwrap_err();
        assert_eq!(unknown.kind(), ErrorKind::InvalidRequest);
    }
}
