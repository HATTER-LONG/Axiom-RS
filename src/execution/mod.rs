//! Immutable execution correlation context.

use crate::foundation::CorrelationId;

/// Explicit parent/child correlation for a request or nested operation.
///
/// The context carries only association identifiers. It is not a dependency
/// container and does not use thread-local or global storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionContext {
    correlation_id: CorrelationId,
    parent_id: Option<CorrelationId>,
}

impl ExecutionContext {
    /// Root context with no parent.
    #[must_use]
    pub fn root(correlation_id: CorrelationId) -> Self {
        Self {
            correlation_id,
            parent_id: None,
        }
    }

    /// Child context whose parent is this context's current identifier.
    ///
    /// The parent context is left unchanged.
    #[must_use]
    pub fn child(&self, correlation_id: CorrelationId) -> Self {
        Self {
            correlation_id,
            parent_id: Some(self.correlation_id.clone()),
        }
    }

    /// Identifier of this context.
    #[must_use]
    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }

    /// Parent identifier, if this context was created with [`ExecutionContext::child`].
    #[must_use]
    pub fn parent_id(&self) -> Option<&CorrelationId> {
        self.parent_id.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(raw: &str) -> CorrelationId {
        CorrelationId::parse(raw).unwrap()
    }

    #[test]
    fn root_has_no_parent() {
        let root = ExecutionContext::root(id("req"));
        assert_eq!(root.correlation_id().as_str(), "req");
        assert!(root.parent_id().is_none());
    }

    #[test]
    fn child_does_not_mutate_parent() {
        let root = ExecutionContext::root(id("req"));
        let child = root.child(id("op"));
        assert_eq!(root.correlation_id().as_str(), "req");
        assert!(root.parent_id().is_none());
        assert_eq!(child.correlation_id().as_str(), "op");
        assert_eq!(child.parent_id().map(CorrelationId::as_str), Some("req"));
    }

    #[test]
    fn nested_propagation() {
        let root = ExecutionContext::root(id("a"));
        let mid = root.child(id("b"));
        let leaf = mid.child(id("c"));
        assert_eq!(leaf.parent_id().map(CorrelationId::as_str), Some("b"));
        assert_eq!(mid.parent_id().map(CorrelationId::as_str), Some("a"));
        assert!(root.parent_id().is_none());
    }
}
