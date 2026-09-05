//! Strict validation of values against [`TypeContract`].

use crate::foundation::{Error, Object, Path, Value};

use super::schema::{FieldContract, TypeContract};

impl TypeContract {
    /// Validate `value` against this contract.
    ///
    /// Successful validation returns the original value unchanged. The first
    /// mismatch, missing field, or unknown field is reported. No implicit
    /// conversions are performed.
    ///
    /// # Errors
    ///
    /// Returns a structured [`Error`] locating the first violation.
    pub fn validate<'a>(&self, value: &'a Value) -> Result<&'a Value, Error> {
        validate_at(self, value, &Path::root())?;
        Ok(value)
    }
}

fn validate_at(contract: &TypeContract, value: &Value, path: &Path) -> Result<(), Error> {
    match contract {
        TypeContract::List(list) => validate_list(&list.item, value, path),
        TypeContract::Object(object) => validate_object(&object.fields, value, path),
        _ => expect_kind(contract, value, path),
    }
}

fn expect_kind(contract: &TypeContract, value: &Value, path: &Path) -> Result<(), Error> {
    if value.kind() == contract.value_kind() {
        Ok(())
    } else {
        Err(Error::type_mismatch(
            path.clone(),
            contract.value_kind(),
            value.kind(),
        ))
    }
}

fn validate_list(item: &TypeContract, value: &Value, path: &Path) -> Result<(), Error> {
    let Some(items) = value.as_list() else {
        return Err(Error::type_mismatch(
            path.clone(),
            contract_kind_list(),
            value.kind(),
        ));
    };
    for (index, entry) in items.iter().enumerate() {
        validate_at(item, entry, &path.index(index))?;
    }
    Ok(())
}

fn contract_kind_list() -> crate::foundation::ValueKind {
    crate::foundation::ValueKind::List
}

fn validate_object(fields: &[FieldContract], value: &Value, path: &Path) -> Result<(), Error> {
    let Some(object) = value.as_object() else {
        return Err(Error::type_mismatch(
            path.clone(),
            TypeContract::object(Vec::new())
                .expect("empty object contract is valid")
                .value_kind(),
            value.kind(),
        ));
    };
    check_required(fields, object, path)?;
    check_unknown(fields, object, path)?;
    check_present(fields, object, path)
}

fn check_required(fields: &[FieldContract], object: &Object, path: &Path) -> Result<(), Error> {
    for field in fields {
        if field.required() && object.get(field.name()).is_none() {
            return Err(Error::missing_field(path.field(field.name()), field.name()));
        }
    }
    Ok(())
}

fn check_unknown(fields: &[FieldContract], object: &Object, path: &Path) -> Result<(), Error> {
    for (name, _) in object.iter() {
        if !fields.iter().any(|field| field.name() == name) {
            return Err(Error::unknown_field(path.field(name), name));
        }
    }
    Ok(())
}

fn check_present(fields: &[FieldContract], object: &Object, path: &Path) -> Result<(), Error> {
    for field in fields {
        if let Some(nested) = object.get(field.name()) {
            let nested_path = path.field(field.name());
            validate_at(field.contract(), nested, &nested_path)?;
            validate_constraints(field, nested, &nested_path)?;
        }
    }
    Ok(())
}

fn validate_constraints(field: &FieldContract, value: &Value, path: &Path) -> Result<(), Error> {
    check_enum(field, value, path)?;
    check_integer_range(field, value, path)?;
    check_float_range(field, value, path)
}

fn check_enum(field: &FieldContract, value: &Value, path: &Path) -> Result<(), Error> {
    let Some(allowed) = field.enum_values() else {
        return Ok(());
    };
    let Some(actual) = value.as_str() else {
        return Ok(());
    };
    if allowed.iter().any(|item| item == actual) {
        Ok(())
    } else {
        Err(Error::constraint_violation(
            path.clone(),
            "enum",
            Value::list(allowed.iter().cloned().map(Value::string)),
            Value::string(actual),
        ))
    }
}

fn check_integer_range(field: &FieldContract, value: &Value, path: &Path) -> Result<(), Error> {
    let Some(actual) = value.as_integer() else {
        return Ok(());
    };
    if let Some(min) = field.integer_min()
        && actual < min
    {
        return Err(range_error(path, "integer_range", actual));
    }
    if let Some(max) = field.integer_max()
        && actual > max
    {
        return Err(range_error(path, "integer_range", actual));
    }
    Ok(())
}

fn check_float_range(field: &FieldContract, value: &Value, path: &Path) -> Result<(), Error> {
    let Some(actual) = value.as_float() else {
        return Ok(());
    };
    if let Some(min) = field.float_min() {
        let violated = if field.float_min_exclusive() {
            actual <= min
        } else {
            actual < min
        };
        if violated {
            return Err(float_range_error(path, actual));
        }
    }
    if let Some(max) = field.float_max() {
        let violated = if field.float_max_exclusive() {
            actual >= max
        } else {
            actual > max
        };
        if violated {
            return Err(float_range_error(path, actual));
        }
    }
    Ok(())
}

fn range_error(path: &Path, constraint: &str, actual: i64) -> Error {
    Error::constraint_violation(
        path.clone(),
        constraint,
        Value::string(constraint),
        Value::integer(actual),
    )
}

fn float_range_error(path: &Path, actual: f64) -> Error {
    Error::constraint_violation(
        path.clone(),
        "float_range",
        Value::string("float_range"),
        Value::try_float(actual).expect("validated floats are finite"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::schema::FieldContract;
    use crate::foundation::{ErrorKind, ValueKind};

    fn sample_contract() -> TypeContract {
        TypeContract::object(vec![
            FieldContract::new("count", TypeContract::Integer, true),
            FieldContract::new(
                "items",
                TypeContract::list(
                    TypeContract::object(vec![FieldContract::new(
                        "name",
                        TypeContract::String,
                        true,
                    )])
                    .unwrap(),
                ),
                true,
            ),
        ])
        .unwrap()
    }

    fn sample_value() -> Value {
        Value::try_object([
            ("count", Value::integer(1)),
            (
                "items",
                Value::list([Value::try_object([("name", Value::from("a"))]).unwrap()]),
            ),
        ])
        .unwrap()
    }

    #[test]
    fn success_does_not_modify_input() {
        let contract = sample_contract();
        let value = sample_value();
        let returned = contract.validate(&value).unwrap();
        assert!(std::ptr::eq(returned, &value));
        assert_eq!(returned, &value);
    }

    #[test]
    fn missing_field() {
        let contract = sample_contract();
        let value = Value::try_object([("count", Value::integer(1))]).unwrap();
        let err = contract.validate(&value).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingField);
        assert_eq!(err.path(), Some(&Path::root().field("items")));
    }

    #[test]
    fn unknown_field() {
        let contract = sample_contract();
        let value = Value::try_object([
            ("count", Value::integer(1)),
            (
                "items",
                Value::list([Value::try_object([("name", Value::from("a"))]).unwrap()]),
            ),
            ("extra", Value::bool(true)),
        ])
        .unwrap();
        let err = contract.validate(&value).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownField);
        assert_eq!(err.path(), Some(&Path::root().field("extra")));
    }

    #[test]
    fn nested_type_mismatch_path() {
        let contract = sample_contract();
        let value = Value::try_object([
            ("count", Value::integer(1)),
            (
                "items",
                Value::list([Value::try_object([("name", Value::integer(1))]).unwrap()]),
            ),
        ])
        .unwrap();
        let err = contract.validate(&value).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::TypeMismatch);
        assert_eq!(
            err.path(),
            Some(&Path::root().field("items").index(0).field("name"))
        );
        let details = err.details().unwrap().as_object().unwrap();
        assert_eq!(
            details.get("expected").and_then(Value::as_str),
            Some("string")
        );
        assert_eq!(
            details.get("actual").and_then(Value::as_str),
            Some("integer")
        );
    }

    #[test]
    fn no_implicit_conversions() {
        let contract = TypeContract::Integer;
        let err = contract.validate(&Value::from("1")).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::TypeMismatch);
        let err = TypeContract::Bool.validate(&Value::integer(1)).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::TypeMismatch);
        assert_eq!(
            TypeContract::Null.validate(&Value::null()).unwrap().kind(),
            ValueKind::Null
        );
    }

    #[test]
    fn first_error_only() {
        let contract = TypeContract::object(vec![
            FieldContract::new("a", TypeContract::Integer, true),
            FieldContract::new("b", TypeContract::Integer, true),
        ])
        .unwrap();
        let value = Value::try_object([("x", Value::bool(true))]).unwrap();
        let err = contract.validate(&value).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingField);
        assert_eq!(err.path(), Some(&Path::root().field("a")));
    }

    #[test]
    fn optional_field_may_be_absent() {
        let contract = TypeContract::object(vec![FieldContract::new(
            "maybe",
            TypeContract::String,
            false,
        )])
        .unwrap();
        let value = Value::try_object::<&str, _>([]).unwrap();
        assert!(contract.validate(&value).is_ok());
    }

    #[test]
    fn enum_and_range_constraints() {
        use crate::contract::FloatRange;
        let contract = TypeContract::object(vec![
            FieldContract::new("unit", TypeContract::String, true)
                .with_enum_values(vec!["mm".into(), "m".into()])
                .unwrap(),
            FieldContract::new("n", TypeContract::Integer, true)
                .with_integer_range(Some(0), Some(10))
                .unwrap(),
            FieldContract::new("x", TypeContract::Float, true)
                .with_float_range(FloatRange {
                    min: Some(0.0),
                    max: Some(1.0),
                    min_exclusive: true,
                    max_exclusive: false,
                })
                .unwrap(),
        ])
        .unwrap();
        let ok = Value::try_object([
            ("unit", Value::from("mm")),
            ("n", Value::integer(0)),
            ("x", Value::try_float(1.0).unwrap()),
        ])
        .unwrap();
        assert!(contract.validate(&ok).is_ok());
        let bad_enum = Value::try_object([
            ("unit", Value::from("cm")),
            ("n", Value::integer(0)),
            ("x", Value::try_float(0.5).unwrap()),
        ])
        .unwrap();
        assert_eq!(
            contract.validate(&bad_enum).unwrap_err().kind(),
            ErrorKind::ConstraintViolation
        );
        let bad_int = Value::try_object([
            ("unit", Value::from("mm")),
            ("n", Value::integer(11)),
            ("x", Value::try_float(0.5).unwrap()),
        ])
        .unwrap();
        assert_eq!(
            contract.validate(&bad_int).unwrap_err().kind(),
            ErrorKind::ConstraintViolation
        );
        let bad_float = Value::try_object([
            ("unit", Value::from("mm")),
            ("n", Value::integer(1)),
            ("x", Value::try_float(0.0).unwrap()),
        ])
        .unwrap();
        assert_eq!(
            contract.validate(&bad_float).unwrap_err().kind(),
            ErrorKind::ConstraintViolation
        );
    }

    #[test]
    fn inclusive_and_exclusive_numeric_bounds() {
        use crate::contract::FloatRange;
        let integers = TypeContract::object(vec![
            FieldContract::new("n", TypeContract::Integer, true)
                .with_integer_range(Some(0), Some(10))
                .unwrap(),
        ])
        .unwrap();
        let int_ok = Value::try_object([("n", Value::integer(10))]).unwrap();
        assert!(integers.validate(&int_ok).is_ok());
        assert!(
            integers
                .validate(&Value::try_object([("n", Value::integer(5))]).unwrap())
                .is_ok()
        );

        let inclusive = TypeContract::object(vec![
            FieldContract::new("x", TypeContract::Float, true)
                .with_float_range(FloatRange {
                    min: Some(0.0),
                    max: Some(1.0),
                    min_exclusive: false,
                    max_exclusive: false,
                })
                .unwrap(),
        ])
        .unwrap();
        assert!(
            inclusive
                .validate(&Value::try_object([("x", Value::try_float(0.0).unwrap())]).unwrap())
                .is_ok()
        );
        assert!(
            inclusive
                .validate(&Value::try_object([("x", Value::try_float(1.0).unwrap())]).unwrap())
                .is_ok()
        );
        assert!(
            inclusive
                .validate(&Value::try_object([("x", Value::try_float(0.5).unwrap())]).unwrap())
                .is_ok()
        );

        let exclusive_max = TypeContract::object(vec![
            FieldContract::new("x", TypeContract::Float, true)
                .with_float_range(FloatRange {
                    min: None,
                    max: Some(1.0),
                    min_exclusive: false,
                    max_exclusive: true,
                })
                .unwrap(),
        ])
        .unwrap();
        assert!(
            exclusive_max
                .validate(&Value::try_object([("x", Value::try_float(0.9).unwrap())]).unwrap())
                .is_ok()
        );
        assert_eq!(
            exclusive_max
                .validate(&Value::try_object([("x", Value::try_float(1.0).unwrap())]).unwrap())
                .unwrap_err()
                .kind(),
            ErrorKind::ConstraintViolation
        );
    }
}
