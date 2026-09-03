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
        TypeContract::List { item } => validate_list(item, value, path),
        TypeContract::Object { fields } => validate_object(fields, value, path),
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
            validate_at(field.contract(), nested, &path.field(field.name()))?;
        }
    }
    Ok(())
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
}
