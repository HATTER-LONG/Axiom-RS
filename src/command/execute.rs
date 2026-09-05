//! Dispatch decoded commands onto [`crate::Runtime`].

use crate::runtime::Runtime;

use super::decode::Command;
use super::encode::CommandResponse;

/// Execute `command` against `runtime`.
///
/// Discovery does not run host implementations. Invoke uses Runtime validation
/// only for capability input.
#[must_use]
pub fn execute(runtime: &Runtime, command: &Command) -> CommandResponse {
    match command {
        Command::List => {
            let listed = runtime.list();
            let values = listed.iter().map(|item| item.to_value());
            CommandResponse::ok(crate::foundation::Value::list(values))
        }
        Command::Get { name } => match runtime.get(name) {
            Some(descriptor) => CommandResponse::ok(descriptor.to_value()),
            None => CommandResponse::ok(crate::foundation::Value::null()),
        },
        Command::Invoke {
            name,
            input,
            context,
        } => {
            let response = match runtime.invoke(name, input.clone(), context) {
                Ok(value) => CommandResponse::ok(value),
                Err(error) => CommandResponse::err(error),
            };
            response.with_context(context)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{Capability, CapabilityCategory, CapabilityDescriptor, CapabilityName};
    use crate::command::decode::decode;
    use crate::contract::TypeContract;
    use crate::foundation::{ErrorKind, Value};

    struct Echo;

    impl Capability for Echo {
        fn invoke(
            &self,
            input: Value,
            _context: &crate::execution::ExecutionContext,
        ) -> Result<Value, crate::capability::BusinessFailure> {
            Ok(input)
        }
    }

    fn runtime() -> Runtime {
        let runtime = Runtime::new();
        runtime
            .register(
                CapabilityDescriptor::new(
                    CapabilityName::parse("echo").unwrap(),
                    "echo integer",
                    CapabilityCategory::parse("test").unwrap(),
                    TypeContract::Integer,
                    TypeContract::Integer,
                )
                .unwrap(),
                Echo,
            )
            .unwrap();
        runtime
    }

    fn obj(fields: &[(&str, Value)]) -> Value {
        Value::try_object(fields.iter().cloned()).unwrap()
    }

    #[test]
    fn list_get_and_invoke_match_native() {
        let runtime = runtime();
        let listed = execute(
            &runtime,
            &decode(&obj(&[
                ("v", Value::integer(1)),
                ("cmd", Value::string("list")),
            ]))
            .unwrap(),
        );
        let names = listed.value().unwrap().as_list().unwrap();
        assert_eq!(names.len(), 1);
        let got = execute(
            &runtime,
            &decode(&obj(&[
                ("v", Value::integer(1)),
                ("cmd", Value::string("get")),
                ("name", Value::string("echo")),
            ]))
            .unwrap(),
        );
        assert_eq!(
            got.value().unwrap().as_object().unwrap().get("name"),
            Some(&Value::string("echo"))
        );
        let missing = execute(
            &runtime,
            &decode(&obj(&[
                ("v", Value::integer(1)),
                ("cmd", Value::string("get")),
                ("name", Value::string("nope")),
            ]))
            .unwrap(),
        );
        assert_eq!(missing.value(), Some(&Value::null()));
        let invoked = execute(
            &runtime,
            &decode(&obj(&[
                ("v", Value::integer(1)),
                ("cmd", Value::string("invoke")),
                ("name", Value::string("echo")),
                ("input", Value::integer(7)),
                ("correlation_id", Value::string("req-1")),
            ]))
            .unwrap(),
        );
        assert_eq!(invoked.value(), Some(&Value::integer(7)));
        assert_eq!(invoked.correlation_id(), Some("req-1"));
        assert!(invoked.is_ok());
        assert!(invoked.parent_id().is_none());
        let nested = execute(
            &runtime,
            &decode(&obj(&[
                ("v", Value::integer(1)),
                ("cmd", Value::string("invoke")),
                ("name", Value::string("echo")),
                ("input", Value::integer(8)),
                ("correlation_id", Value::string("child")),
                ("parent_id", Value::string("root")),
            ]))
            .unwrap(),
        );
        assert_eq!(nested.parent_id(), Some("root"));
        assert_eq!(nested.correlation_id(), Some("child"));
    }

    #[test]
    fn invoke_unknown_is_error() {
        let runtime = runtime();
        let response = execute(
            &runtime,
            &decode(&obj(&[
                ("v", Value::integer(1)),
                ("cmd", Value::string("invoke")),
                ("name", Value::string("missing")),
                ("input", Value::null()),
                ("correlation_id", Value::string("req-1")),
            ]))
            .unwrap(),
        );
        assert_eq!(
            response.error().unwrap().kind(),
            ErrorKind::UnknownCapability
        );
    }
}
