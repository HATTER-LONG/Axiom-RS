//! Host implementation boundary and business failures.

use crate::execution::ExecutionContext;
use crate::foundation::Value;

/// Host-owned failure that Runtime records as [`crate::ErrorKind::BusinessFailure`].
///
/// Callers cannot assemble an arbitrary [`crate::Error`] from this type. Path
/// and kind are filled by Runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BusinessFailure {
    message: String,
    details: Option<Value>,
}

impl BusinessFailure {
    /// Failure with a non-empty message.
    ///
    /// # Panics
    ///
    /// Panics when `message` is empty or only whitespace. That is a host
    /// programming defect, not a runtime input error.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        let message = message.into();
        assert!(
            !is_blank(&message),
            "business failure message must not be blank"
        );
        Self {
            message,
            details: None,
        }
    }

    /// Attach structured details preserved in the Runtime error.
    #[must_use]
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    #[must_use]
    pub(crate) fn into_parts(self) -> (String, Option<Value>) {
        (self.message, self.details)
    }
}

fn is_blank(value: &str) -> bool {
    value.is_empty() || value.chars().all(char::is_whitespace)
}

/// Synchronous host implementation of one capability.
///
/// The trait is not `Send` or `Sync`. Runtime invokes it on the thread that
/// owns the runtime after input validation and without holding registry
/// borrows.
pub trait Capability {
    /// Execute the capability.
    ///
    /// `input` has already been validated against the registered input
    /// contract. Return owned output or a business failure. Do not construct
    /// Axiom [`crate::Error`] values.
    fn invoke(&self, input: Value, context: &ExecutionContext) -> Result<Value, BusinessFailure>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::CorrelationId;

    struct Echo;

    impl Capability for Echo {
        fn invoke(
            &self,
            input: Value,
            context: &ExecutionContext,
        ) -> Result<Value, BusinessFailure> {
            assert_eq!(context.correlation_id().as_str(), "c");
            Ok(input)
        }
    }

    #[test]
    fn host_sees_original_context() {
        let ctx = ExecutionContext::root(CorrelationId::parse("c").unwrap());
        let out = Echo.invoke(Value::integer(1), &ctx).unwrap();
        assert_eq!(out, Value::integer(1));
        let fail = BusinessFailure::new("nope").with_details(Value::integer(2));
        let (message, details) = fail.into_parts();
        assert_eq!(message, "nope");
        assert_eq!(details, Some(Value::integer(2)));
    }

    #[test]
    #[should_panic(expected = "must not be blank")]
    fn blank_message_is_a_host_defect() {
        let _ = BusinessFailure::new("");
    }

    #[test]
    #[should_panic(expected = "must not be blank")]
    fn whitespace_message_is_a_host_defect() {
        let _ = BusinessFailure::new(" \t");
    }
}
