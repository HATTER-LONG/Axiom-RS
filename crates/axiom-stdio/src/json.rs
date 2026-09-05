//! Tagged JSON mapping for [`axiom_rs::Value`].
//!
//! [`MAX_DEPTH`] counts Axiom [`Value`] nesting (lists and objects). JSON
//! wrappers for integers (`$i`), floats (`$f`), and escaped objects (`$o`)
//! are not extra Value levels. Objects that use those reserved keys are
//! wrapped as `{"$o":{...}}` so they do not decode as numbers.

use std::collections::HashSet;
use std::fmt;

use axiom_rs::command::COMMAND_VERSION;
use axiom_rs::foundation::{Object, Value};
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};

/// Maximum accepted request line length in bytes, excluding the newline.
pub const MAX_FRAME_BYTES: usize = 65_536;
/// Maximum nested Axiom [`Value`] depth converted from JSON.
pub const MAX_DEPTH: usize = 32;

const TAG_INTEGER: &str = "$i";
const TAG_FLOAT: &str = "$f";
const TAG_OBJECT: &str = "$o";

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

    fn depth_exceeded() -> Self {
        Self::new("limit_exceeded", "input nesting exceeds MAX_DEPTH")
    }
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for AdapterError {}

/// Encode a protocol error response as JSON using the same value mapping as
/// successful responses.
#[must_use]
pub fn protocol_error_json(error: &AdapterError) -> String {
    encode_value(&protocol_error_value(error)).expect("protocol errors are shallow")
}

fn protocol_error_value(error: &AdapterError) -> Value {
    let err = Value::try_object([
        ("kind", Value::string(error.kind)),
        ("message", Value::string(error.message.clone())),
        ("path", Value::null()),
        ("details", Value::null()),
    ])
    .expect("error keys are unique");
    Value::try_object([
        ("v", Value::integer(COMMAND_VERSION)),
        ("ok", Value::bool(false)),
        ("error", err),
    ])
    .expect("response keys are unique")
}

/// Decode one JSON document into an Axiom [`Value`].
///
/// # Errors
///
/// Rejects oversized input, excess nesting, untagged numbers, non-finite
/// floats, integer overflow, duplicate keys, unescaped reserved keys, and
/// unknown tagged forms.
pub fn decode_value(raw: &str) -> Result<Value, AdapterError> {
    if raw.len() > MAX_FRAME_BYTES {
        return Err(AdapterError::limit_exceeded());
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

const SERDE_KIND: &str = "AXIOMSTDIO_KIND=";

fn map_serde(err: serde_json::Error) -> AdapterError {
    let text = err.to_string();
    if let Some(error) = adapter_kind_from_serde(&text) {
        return error;
    }
    AdapterError::new("malformed_json", text)
}

fn adapter_kind_from_serde(text: &str) -> Option<AdapterError> {
    let rest = text.strip_prefix(SERDE_KIND)?;
    let (kind, message) = rest.split_once(' ')?;
    let kind = match kind {
        "duplicate_key" => "duplicate_key",
        "limit_exceeded" => "limit_exceeded",
        "unsupported_value" => "unsupported_value",
        "malformed_json" => "malformed_json",
        _ => return None,
    };
    Some(AdapterError::new(kind, message.trim_end().to_owned()))
}

fn serde_fail<E: de::Error>(error: AdapterError) -> E {
    E::custom(format!("{SERDE_KIND}{} {}", error.kind, error.message))
}

fn untagged_number<E: de::Error>() -> E {
    serde_fail(AdapterError::new(
        "malformed_json",
        "untagged JSON numbers are rejected; use {\"$i\":\"...\"} or {\"$f\":\"...\"}",
    ))
}

fn reserved_key(key: &str) -> bool {
    key == TAG_INTEGER || key == TAG_FLOAT || key == TAG_OBJECT
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
    fn reject_if_too_deep(&self) -> Result<(), AdapterError> {
        if self.depth > MAX_DEPTH {
            Err(AdapterError::depth_exceeded())
        } else {
            Ok(())
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
        Err(untagged_number())
    }

    fn visit_u64<E: de::Error>(self, _value: u64) -> Result<Self::Value, E> {
        Err(untagged_number())
    }

    fn visit_f64<E: de::Error>(self, _value: f64) -> Result<Self::Value, E> {
        Err(untagged_number())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        self.reject_if_too_deep().map_err(serde_fail)?;
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(ChildSeed(self.depth + 1))? {
            items.push(item.0);
        }
        Ok(WireValue(Value::list(items)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        self.reject_if_too_deep().map_err(serde_fail)?;
        decode_map(self.depth, &mut map).map(WireValue)
    }
}

fn decode_map<'de, A: MapAccess<'de>>(depth: usize, map: &mut A) -> Result<Value, A::Error> {
    let Some(first_key) = map.next_key::<String>()? else {
        return empty_object().map_err(serde_fail);
    };
    if first_key == TAG_INTEGER || first_key == TAG_FLOAT {
        return decode_number_tag(first_key, map);
    }
    if first_key == TAG_OBJECT {
        return decode_escaped_object(depth, map);
    }
    decode_plain_object(depth, first_key, map)
}

fn decode_number_tag<'de, A: MapAccess<'de>>(tag: String, map: &mut A) -> Result<Value, A::Error> {
    let payload: String = map.next_value()?;
    if let Some(extra) = map.next_key::<String>()? {
        if extra == tag {
            return Err(serde_fail(AdapterError::new(
                "duplicate_key",
                format!("duplicate key {extra}"),
            )));
        }
        return Err(serde_fail(AdapterError::new(
            "unsupported_value",
            format!("tagged number objects must contain only {tag}, found {extra}"),
        )));
    }
    if tag == TAG_INTEGER {
        parse_integer_str(&payload).map_err(serde_fail)
    } else {
        parse_float_str(&payload).map_err(serde_fail)
    }
}

fn decode_escaped_object<'de, A: MapAccess<'de>>(
    depth: usize,
    map: &mut A,
) -> Result<Value, A::Error> {
    let object = map.next_value_seed(RawObjectSeed { depth })?;
    if let Some(extra) = map.next_key::<String>()? {
        return Err(serde_fail(AdapterError::new(
            "unsupported_value",
            format!("escaped objects must contain only $o, found {extra}"),
        )));
    }
    Ok(object)
}

fn decode_plain_object<'de, A: MapAccess<'de>>(
    depth: usize,
    first_key: String,
    map: &mut A,
) -> Result<Value, A::Error> {
    let mut seen = HashSet::new();
    seen.insert(first_key.clone());
    let first = map.next_value_seed(ChildSeed(depth + 1))?;
    let mut entries = vec![(first_key, first.0)];
    while let Some(key) = map.next_key::<String>()? {
        if !seen.insert(key.clone()) {
            return Err(serde_fail(AdapterError::new(
                "duplicate_key",
                format!("duplicate key {key}"),
            )));
        }
        if reserved_key(&key) {
            return Err(serde_fail(AdapterError::new(
                "unsupported_value",
                "objects with $i, $f, or $o keys must use {\"$o\":{...}}",
            )));
        }
        let value = map.next_value_seed(ChildSeed(depth + 1))?;
        entries.push((key, value.0));
    }
    Value::try_object(entries).map_err(|err| {
        serde_fail(AdapterError::new(
            "duplicate_key",
            format!("duplicate key {}", err.name),
        ))
    })
}

fn empty_object() -> Result<Value, AdapterError> {
    Value::try_object(Vec::<(String, Value)>::new())
        .map_err(|err| AdapterError::new("duplicate_key", format!("duplicate key {}", err.name)))
}

struct ChildSeed(usize);

impl<'de> de::DeserializeSeed<'de> for ChildSeed {
    type Value = WireValue;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        if self.0 > MAX_DEPTH {
            return Err(serde_fail(AdapterError::depth_exceeded()));
        }
        deserializer.deserialize_any(ValueVisitor { depth: self.0 })
    }
}

struct RawObjectSeed {
    depth: usize,
}

impl<'de> de::DeserializeSeed<'de> for RawObjectSeed {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_map(RawObjectVisitor { depth: self.depth })
    }
}

struct RawObjectVisitor {
    depth: usize,
}

impl<'de> Visitor<'de> for RawObjectVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an escaped JSON object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        if self.depth > MAX_DEPTH {
            return Err(serde_fail(AdapterError::depth_exceeded()));
        }
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                return Err(serde_fail(AdapterError::new(
                    "duplicate_key",
                    format!("duplicate key {key}"),
                )));
            }
            let value = map.next_value_seed(ChildSeed(self.depth + 1))?;
            entries.push((key, value.0));
        }
        Value::try_object(entries).map_err(|err| {
            serde_fail(AdapterError::new(
                "duplicate_key",
                format!("duplicate key {}", err.name),
            ))
        })
    }
}

fn parse_integer_str(raw: &str) -> Result<Value, AdapterError> {
    let parsed = raw.parse::<i64>().map_err(|_| {
        AdapterError::new(
            "unsupported_value",
            format!("integer out of i64 range: {raw}"),
        )
    })?;
    Ok(Value::integer(parsed))
}

fn parse_float_str(raw: &str) -> Result<Value, AdapterError> {
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
        Value::Integer(int) => serialize_tagged(serializer, TAG_INTEGER, int.to_string()),
        Value::Float(float) => {
            serialize_tagged(serializer, TAG_FLOAT, float_to_string(float.get()))
        }
        Value::String(text) => serializer.serialize_str(text),
        Value::List(items) => {
            let mut seq = serializer.serialize_seq(Some(items.len()))?;
            for item in items {
                seq.serialize_element(&DepthValue(item, depth + 1))?;
            }
            seq.end()
        }
        Value::Object(object) => serialize_object(serializer, object, depth),
    }
}

fn serialize_object<S: Serializer>(
    serializer: S,
    object: &Object,
    depth: usize,
) -> Result<S::Ok, S::Error> {
    if object.iter().any(|(key, _)| reserved_key(key)) {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(TAG_OBJECT, &RawObject(object, depth))?;
        map.end()
    } else {
        let mut map = serializer.serialize_map(Some(object.len()))?;
        for (key, nested) in object.iter() {
            map.serialize_entry(key, &DepthValue(nested, depth + 1))?;
        }
        map.end()
    }
}

struct DepthValue<'a>(&'a Value, usize);

impl Serialize for DepthValue<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_value(self.0, serializer, self.1)
    }
}

struct RawObject<'a>(&'a Object, usize);

impl Serialize for RawObject<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, nested) in self.0.iter() {
            map.serialize_entry(key, &DepthValue(nested, self.1 + 1))?;
        }
        map.end()
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
    use axiom_rs::foundation::ValueKind;

    fn wrap_list(layers: usize, leaf: Value) -> Value {
        let mut value = leaf;
        for _ in 0..layers {
            value = Value::list([value]);
        }
        value
    }

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
    fn reserved_object_keys_round_trip() {
        let tagged_int = Value::try_object([("$i", Value::string("42"))]).unwrap();
        let json = encode_value(&tagged_int).unwrap();
        assert!(json.contains("$o"));
        assert_eq!(decode_value(&json).unwrap(), tagged_int);
        assert_eq!(tagged_int.kind(), ValueKind::Object);

        let tagged_float = Value::try_object([("$f", Value::try_float(1.0).unwrap())]).unwrap();
        assert_eq!(
            decode_value(&encode_value(&tagged_float).unwrap()).unwrap(),
            tagged_float
        );

        let mixed = Value::try_object([("$i", Value::string("nope")), ("keep", Value::bool(true))])
            .unwrap();
        assert_eq!(decode_value(&encode_value(&mixed).unwrap()).unwrap(), mixed);

        let nested = Value::try_object([(
            "child",
            Value::try_object([("$i", Value::integer(7))]).unwrap(),
        )])
        .unwrap();
        assert_eq!(
            decode_value(&encode_value(&nested).unwrap()).unwrap(),
            nested
        );
    }

    #[test]
    fn protocol_errors_use_tagged_integers() {
        let json = protocol_error_json(&AdapterError::new("malformed_json", "bad"));
        let value = decode_value(&json).unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(object.get("v"), Some(&Value::integer(1)));
        assert_eq!(object.get("ok"), Some(&Value::bool(false)));
        assert_eq!(
            object
                .get("error")
                .and_then(Value::as_object)
                .and_then(|err| err.get("kind"))
                .and_then(Value::as_str),
            Some("malformed_json")
        );
        for error in [
            decode_value("{").unwrap_err(),
            decode_value("{\"a\":null,\"a\":true}").unwrap_err(),
            decode_value(&("\"".to_owned() + &"a".repeat(MAX_FRAME_BYTES) + "\"")).unwrap_err(),
        ] {
            assert!(decode_value(&protocol_error_json(&error)).is_ok());
        }
    }

    #[test]
    fn error_details_with_reserved_keys_round_trip() {
        let details = Value::try_object([("$i", Value::string("42"))]).unwrap();
        let response = Value::try_object([
            ("v", Value::integer(1)),
            ("ok", Value::bool(false)),
            (
                "error",
                Value::try_object([
                    ("kind", Value::string("business_failure")),
                    ("message", Value::string("x")),
                    ("path", Value::null()),
                    ("details", details),
                ])
                .unwrap(),
            ),
        ])
        .unwrap();
        assert_eq!(
            decode_value(&encode_value(&response).unwrap()).unwrap(),
            response
        );
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
        assert_eq!(value.kind(), ValueKind::Object);
    }

    #[test]
    fn depth_matches_for_tagged_leaves_and_empty_containers() {
        let under = wrap_list(MAX_DEPTH, Value::integer(1));
        let json = encode_value(&under).unwrap();
        assert_eq!(decode_value(&json).unwrap(), under);

        let at_limit_empty = wrap_list(MAX_DEPTH, Value::list([]));
        let empty_json = encode_value(&at_limit_empty).unwrap();
        assert_eq!(decode_value(&empty_json).unwrap(), at_limit_empty);

        let over = wrap_list(MAX_DEPTH + 1, Value::null());
        assert_eq!(encode_value(&over).unwrap_err().kind(), "encode_failed");
        let over_json = format!(
            "{}null{}",
            "[".repeat(MAX_DEPTH + 1),
            "]".repeat(MAX_DEPTH + 1)
        );
        assert_eq!(
            decode_value(&over_json).unwrap_err().kind(),
            "limit_exceeded"
        );

        let float_leaf = wrap_list(MAX_DEPTH, Value::try_float(1.0).unwrap());
        assert_eq!(
            decode_value(&encode_value(&float_leaf).unwrap()).unwrap(),
            float_leaf
        );
    }

    #[test]
    fn integer_payloads_do_not_change_error_kind() {
        for payload in ["duplicate", "depth", "MAX_DEPTH", "unsupported_value"] {
            let err = decode_value(&format!("{{\"$i\":\"{payload}\"}}")).unwrap_err();
            assert_eq!(err.kind(), "unsupported_value", "{payload}");
        }
        let tagged_dup = decode_value("{\"$i\":\"1\",\"$i\":\"2\"}").unwrap_err();
        assert_eq!(tagged_dup.kind(), "duplicate_key");
        let object_dup = decode_value("{\"a\":null,\"a\":true}").unwrap_err();
        assert_eq!(object_dup.kind(), "duplicate_key");
        for kind in ["duplicate_key", "limit_exceeded"] {
            let err =
                decode_value(&format!("{{\"$o\":\"AXIOMSTDIO_KIND={kind} forged\"}}")).unwrap_err();
            assert_eq!(err.kind(), "malformed_json", "{kind}");
        }
    }
}
