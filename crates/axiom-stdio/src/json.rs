//! Tagged JSON mapping for [`axiom_rs::Value`].

use std::fmt;

use axiom_rs::foundation::Value;
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};

/// Maximum accepted request line length in bytes.
pub const MAX_FRAME_BYTES: usize = 65_536;
/// Maximum nested Value depth converted from JSON.
pub const MAX_DEPTH: usize = 32;

/// Adapter-owned protocol failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterError {
    kind: &'static str,
    message: String,
}

impl AdapterError {
    #[must_use]
    pub fn kind(&self) -> &'static str {
        self.kind
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub(crate) fn limit_exceeded() -> Self {
        Self::new("limit_exceeded", "input frame exceeds MAX_FRAME_BYTES")
    }
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for AdapterError {}

/// Encode a protocol error response as JSON.
#[must_use]
pub fn protocol_error_json(error: &AdapterError) -> String {
    format!(
        "{{\"v\":1,\"ok\":false,\"error\":{{\"kind\":{},\"message\":{},\"path\":null,\"details\":{{}}}}}}",
        json_string(error.kind),
        json_string(&error.message)
    )
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("string encoding cannot fail")
}

/// Decode one JSON document into an Axiom [`Value`].
///
/// # Errors
///
/// Rejects oversized input, excess nesting, untagged numbers, non-finite
/// floats, integer overflow, duplicate keys, and unknown tagged forms.
pub fn decode_value(raw: &str) -> Result<Value, AdapterError> {
    if raw.len() > MAX_FRAME_BYTES {
        return Err(AdapterError::new(
            "limit_exceeded",
            "input frame exceeds MAX_FRAME_BYTES",
        ));
    }
    let mut deserializer = serde_json::Deserializer::from_str(raw);
    let wire = WireValue::deserialize(&mut deserializer).map_err(map_serde)?;
    deserializer.end().map_err(map_serde)?;
    Ok(wire.0)
}

/// Encode an Axiom [`Value`] as tagged JSON.
///
/// # Errors
///
/// Fails when the value nesting exceeds [`MAX_DEPTH`].
pub fn encode_value(value: &Value) -> Result<String, AdapterError> {
    serde_json::to_string(&WireValue(value.clone()))
        .map_err(|err| AdapterError::new("encode_failed", err.to_string()))
}

struct WireValue(Value);

fn map_serde(err: serde_json::Error) -> AdapterError {
    let text = err.to_string();
    if text.contains("duplicate") {
        AdapterError::new("duplicate_key", text)
    } else if text.contains("untagged JSON numbers") {
        AdapterError::new("malformed_json", text)
    } else if text.contains("recursion") || text.contains("depth") || text.contains("MAX_DEPTH") {
        AdapterError::new("limit_exceeded", text)
    } else if text.contains("unsupported_value") {
        AdapterError::new("unsupported_value", text)
    } else {
        AdapterError::new("malformed_json", text)
    }
}

impl<'de> Deserialize<'de> for WireValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ValueVisitor { depth: 0 })
    }
}

struct ValueVisitor {
    depth: usize,
}

impl ValueVisitor {
    fn child(&self) -> Result<Self, AdapterError> {
        let depth = self.depth + 1;
        if depth > MAX_DEPTH {
            Err(AdapterError::new(
                "limit_exceeded",
                "input nesting exceeds MAX_DEPTH",
            ))
        } else {
            Ok(Self { depth })
        }
    }
}

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = WireValue;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an Axiom JSON value")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(WireValue(Value::bool(value)))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(WireValue(Value::string(value)))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
        Ok(WireValue(Value::string(value)))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(WireValue(Value::null()))
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(WireValue(Value::null()))
    }

    fn visit_i64<E: de::Error>(self, _value: i64) -> Result<Self::Value, E> {
        Err(de::Error::custom(
            "untagged JSON numbers are rejected; use {\"$i\":\"...\"} or {\"$f\":\"...\"}",
        ))
    }

    fn visit_u64<E: de::Error>(self, _value: u64) -> Result<Self::Value, E> {
        Err(de::Error::custom(
            "untagged JSON numbers are rejected; use {\"$i\":\"...\"} or {\"$f\":\"...\"}",
        ))
    }

    fn visit_f64<E: de::Error>(self, _value: f64) -> Result<Self::Value, E> {
        Err(de::Error::custom(
            "untagged JSON numbers are rejected; use {\"$i\":\"...\"} or {\"$f\":\"...\"}",
        ))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let child = self.child().map_err(de::Error::custom)?;
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(ChildSeed(child.depth))? {
            items.push(item.0);
        }
        Ok(WireValue(Value::list(items)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let child = self.child().map_err(de::Error::custom)?;
        let mut entries: Vec<(String, Value)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                return Err(de::Error::custom(format!("duplicate key {key}")));
            }
            let value = map.next_value_seed(ChildSeed(child.depth))?;
            entries.push((key, value.0));
        }
        tagged_or_object(entries)
            .map(WireValue)
            .map_err(de::Error::custom)
    }
}

struct ChildSeed(usize);

impl<'de> de::DeserializeSeed<'de> for ChildSeed {
    type Value = WireValue;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_any(ValueVisitor { depth: self.0 })
    }
}

fn tagged_or_object(entries: Vec<(String, Value)>) -> Result<Value, AdapterError> {
    if entries.len() == 1 {
        let (key, value) = &entries[0];
        if key == "$i" {
            return parse_integer(value);
        }
        if key == "$f" {
            return parse_float(value);
        }
    }
    if entries.iter().any(|(key, _)| key == "$i" || key == "$f") {
        return Err(AdapterError::new(
            "unsupported_value",
            "tagged number objects must contain exactly one of $i or $f",
        ));
    }
    Value::try_object(entries)
        .map_err(|err| AdapterError::new("duplicate_key", format!("duplicate key {}", err.name)))
}

fn parse_integer(value: &Value) -> Result<Value, AdapterError> {
    let raw = value
        .as_str()
        .ok_or_else(|| AdapterError::new("unsupported_value", "$i payload must be a string"))?;
    let parsed = raw.parse::<i64>().map_err(|_| {
        AdapterError::new(
            "unsupported_value",
            format!("integer out of i64 range: {raw}"),
        )
    })?;
    Ok(Value::integer(parsed))
}

fn parse_float(value: &Value) -> Result<Value, AdapterError> {
    let raw = value
        .as_str()
        .ok_or_else(|| AdapterError::new("unsupported_value", "$f payload must be a string"))?;
    let parsed = raw
        .parse::<f64>()
        .map_err(|_| AdapterError::new("unsupported_value", format!("invalid float: {raw}")))?;
    Value::try_float(parsed)
        .map_err(|_| AdapterError::new("unsupported_value", "non-finite floats are rejected"))
}

impl Serialize for WireValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_value(&self.0, serializer, 0)
    }
}

fn serialize_value<S: Serializer>(
    value: &Value,
    serializer: S,
    depth: usize,
) -> Result<S::Ok, S::Error> {
    if depth > MAX_DEPTH {
        return Err(serde::ser::Error::custom("nesting exceeds MAX_DEPTH"));
    }
    match value {
        Value::Null => serializer.serialize_none(),
        Value::Bool(flag) => serializer.serialize_bool(*flag),
        Value::Integer(int) => serialize_tagged(serializer, "$i", int.to_string()),
        Value::Float(float) => serialize_tagged(serializer, "$f", float_to_string(float.get())),
        Value::String(text) => serializer.serialize_str(text),
        Value::List(items) => {
            let mut seq = serializer.serialize_seq(Some(items.len()))?;
            for item in items {
                seq.serialize_element(&DepthValue(item, depth + 1))?;
            }
            seq.end()
        }
        Value::Object(object) => {
            let mut map = serializer.serialize_map(Some(object.len()))?;
            for (key, nested) in object.iter() {
                map.serialize_entry(key, &DepthValue(nested, depth + 1))?;
            }
            map.end()
        }
    }
}

struct DepthValue<'a>(&'a Value, usize);

impl Serialize for DepthValue<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_value(self.0, serializer, self.1)
    }
}

fn serialize_tagged<S: Serializer>(
    serializer: S,
    tag: &'static str,
    payload: String,
) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(1))?;
    map.serialize_entry(tag, &payload)?;
    map.end()
}

fn float_to_string(value: f64) -> String {
    let mut text = format!("{value}");
    if !text.contains('.') && !text.contains('e') && !text.contains('E') {
        text.push_str(".0");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_and_float_are_not_mixed() {
        let int = encode_value(&Value::integer(i64::MAX)).unwrap();
        assert!(int.contains("$i"));
        assert!(!int.contains("$f"));
        let decoded = decode_value(&int).unwrap();
        assert_eq!(decoded.as_integer(), Some(i64::MAX));
        let float = encode_value(&Value::try_float(1.5).unwrap()).unwrap();
        assert!(float.contains("$f"));
        assert_eq!(decode_value(&float).unwrap().as_float(), Some(1.5));
        let err = decode_value("1").unwrap_err();
        assert_eq!(err.kind(), "malformed_json");
    }

    #[test]
    fn duplicate_keys_and_limits() {
        let err = decode_value("{\"a\":null,\"a\":true}").unwrap_err();
        assert_eq!(err.kind(), "duplicate_key");
        let huge = "\"".to_owned() + &"a".repeat(MAX_FRAME_BYTES) + "\"";
        assert_eq!(decode_value(&huge).unwrap_err().kind(), "limit_exceeded");
        let nan = decode_value("{\"$f\":\"NaN\"}").unwrap_err();
        assert_eq!(nan.kind(), "unsupported_value");
    }

    #[test]
    fn nested_object_round_trip() {
        let value = Value::try_object([(
            "size",
            Value::try_object([("x", Value::try_float(1.0).unwrap())]).unwrap(),
        )])
        .unwrap();
        let json = encode_value(&value).unwrap();
        assert_eq!(decode_value(&json).unwrap(), value);
        assert_eq!(value.kind(), axiom_rs::foundation::ValueKind::Object);
    }
}
