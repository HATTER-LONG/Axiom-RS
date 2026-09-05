//! Discovery encoding of contracts as [`Value`].

use crate::foundation::Value;

use super::schema::{FieldContract, TypeContract};

impl TypeContract {
    /// Machine-readable description of this contract.
    ///
    /// Command discovery and native accessors share this encoding. Adapter
    /// layers must not invent a second rule set.
    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            Self::Null => kind_only("null"),
            Self::Bool => kind_only("bool"),
            Self::Integer => kind_only("integer"),
            Self::Float => kind_only("float"),
            Self::String => kind_only("string"),
            Self::List(list) => object_value(&[
                ("kind", Value::string("list")),
                ("item", list.item.to_value()),
            ]),
            Self::Object(object) => object_value(&[
                ("kind", Value::string("object")),
                (
                    "fields",
                    Value::list(object.fields.iter().map(FieldContract::to_value)),
                ),
            ]),
        }
    }
}

impl FieldContract {
    /// Machine-readable field description used by discovery.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut fields = vec![
            ("name", Value::string(self.name())),
            ("required", Value::bool(self.required())),
            ("contract", self.contract().to_value()),
        ];
        push_optional(&mut fields, "description", self.description());
        push_optional(&mut fields, "unit", self.unit());
        if let Some(values) = self.enum_values() {
            fields.push((
                "enum",
                Value::list(values.iter().cloned().map(Value::string)),
            ));
        }
        push_integer(&mut fields, "integer_min", self.integer_min());
        push_integer(&mut fields, "integer_max", self.integer_max());
        push_float(&mut fields, "float_min", self.float_min());
        push_float(&mut fields, "float_max", self.float_max());
        if self.float_min().is_some() {
            fields.push((
                "float_min_exclusive",
                Value::bool(self.float_min_exclusive()),
            ));
        }
        if self.float_max().is_some() {
            fields.push((
                "float_max_exclusive",
                Value::bool(self.float_max_exclusive()),
            ));
        }
        object_value(&fields)
    }
}

fn kind_only(kind: &str) -> Value {
    object_value(&[("kind", Value::string(kind))])
}

fn push_optional<'a>(fields: &mut Vec<(&'a str, Value)>, key: &'a str, value: Option<&str>) {
    if let Some(value) = value {
        fields.push((key, Value::string(value)));
    }
}

fn push_integer<'a>(fields: &mut Vec<(&'a str, Value)>, key: &'a str, value: Option<i64>) {
    if let Some(value) = value {
        fields.push((key, Value::integer(value)));
    }
}

fn push_float<'a>(fields: &mut Vec<(&'a str, Value)>, key: &'a str, value: Option<f64>) {
    if let Some(value) = value {
        fields.push((
            key,
            Value::try_float(value).expect("stored bounds are finite"),
        ));
    }
}

fn object_value(fields: &[(&str, Value)]) -> Value {
    Value::try_object(fields.iter().cloned()).expect("discovery keys are unique")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::schema::{FieldContract, FloatRange};

    #[test]
    fn nested_object_encoding_includes_constraints() {
        let field = FieldContract::new("unit", TypeContract::String, true)
            .with_description("length unit")
            .unwrap()
            .with_enum_values(vec!["mm".into(), "m".into()])
            .unwrap();
        let encoded = field.to_value();
        let object = encoded.as_object().unwrap();
        assert_eq!(
            object.get("description").unwrap().as_str(),
            Some("length unit")
        );
        assert_eq!(
            object.get("enum").unwrap().as_list().unwrap(),
            &[Value::string("mm"), Value::string("m")]
        );
        let ranged = FieldContract::new("x", TypeContract::Float, true)
            .with_float_range(FloatRange {
                min: Some(0.0),
                max: None,
                min_exclusive: true,
                max_exclusive: false,
            })
            .unwrap()
            .to_value();
        let range_obj = ranged.as_object().unwrap();
        assert_eq!(
            range_obj.get("float_min_exclusive").unwrap().as_bool(),
            Some(true)
        );
        assert_eq!(
            TypeContract::Integer
                .to_value()
                .as_object()
                .unwrap()
                .get("kind"),
            Some(&Value::string("integer"))
        );
        let numbered = FieldContract::new("n", TypeContract::Integer, true)
            .with_integer_range(Some(-1), Some(8))
            .unwrap()
            .to_value();
        let numbered_obj = numbered.as_object().unwrap();
        assert_eq!(numbered_obj.get("integer_min"), Some(&Value::integer(-1)));
        assert_eq!(numbered_obj.get("integer_max"), Some(&Value::integer(8)));
        let floats = FieldContract::new("x", TypeContract::Float, true)
            .with_float_range(FloatRange {
                min: Some(0.5),
                max: Some(2.5),
                min_exclusive: false,
                max_exclusive: true,
            })
            .unwrap()
            .to_value();
        let float_obj = floats.as_object().unwrap();
        assert_eq!(
            float_obj.get("float_min").and_then(Value::as_float),
            Some(0.5)
        );
        assert_eq!(
            float_obj.get("float_max").and_then(Value::as_float),
            Some(2.5)
        );
        assert!(
            plain_field()
                .as_object()
                .unwrap()
                .get("integer_min")
                .is_none()
        );
        assert!(
            plain_field()
                .as_object()
                .unwrap()
                .get("float_min")
                .is_none()
        );
    }

    fn plain_field() -> Value {
        FieldContract::new("n", TypeContract::Integer, true).to_value()
    }
}
