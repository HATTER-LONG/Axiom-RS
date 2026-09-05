//! Thread-affine executable registry and synchronous invocation.
//!
//! [`Runtime`] is a cloneable handle sharing one capability table on the
//! thread that created it. Descriptor and implementation register together.
//! [`crate::CapabilityRegistry`] is not consulted.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::thread::ThreadId;

use crate::capability::{Capability, CapabilityDescriptor, CapabilityName, duplicate_capability};
use crate::contract::TypeContract;
use crate::execution::ExecutionContext;
use crate::foundation::{Error, Value};

/// Authoritative store of executable capabilities.
#[derive(Clone, Debug)]
pub struct Runtime {
    owner: ThreadId,
    inner: Rc<Inner>,
}

#[derive(Debug)]
struct Inner {
    entries: RefCell<BTreeMap<CapabilityName, Entry>>,
}

struct Entry {
    descriptor: CapabilityDescriptor,
    capability: Rc<dyn Capability>,
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("descriptor", &self.descriptor)
            .finish_non_exhaustive()
    }
}

impl Runtime {
    /// Empty runtime bound to the current thread.
    #[must_use]
    pub fn new() -> Self {
        Self {
            owner: std::thread::current().id(),
            inner: Rc::new(Inner {
                entries: RefCell::new(BTreeMap::new()),
            }),
        }
    }

    /// Register `descriptor` and `capability` as one executable item.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorKind::DuplicateCapability`] when the name is
    /// already present. The previous descriptor and implementation remain.
    ///
    /// # Panics
    ///
    /// Panics when called from a thread other than the creator thread.
    pub fn register(
        &self,
        descriptor: CapabilityDescriptor,
        capability: impl Capability + 'static,
    ) -> Result<(), Error> {
        self.assert_owner();
        let name = descriptor.name().clone();
        let mut entries = self.inner.entries.borrow_mut();
        if entries.contains_key(&name) {
            return Err(duplicate_capability(name.as_str()));
        }
        entries.insert(
            name,
            Entry {
                descriptor,
                capability: Rc::new(capability),
            },
        );
        Ok(())
    }

    /// Owned snapshot of a registered descriptor, if any. Does not run host code.
    ///
    /// # Panics
    ///
    /// Panics when called from a thread other than the creator thread.
    #[must_use]
    pub fn get(&self, name: &CapabilityName) -> Option<CapabilityDescriptor> {
        self.assert_owner();
        self.inner
            .entries
            .borrow()
            .get(name)
            .map(|entry| entry.descriptor.clone())
    }

    /// Owned snapshots ordered by capability name. Does not run host code.
    ///
    /// # Panics
    ///
    /// Panics when called from a thread other than the creator thread.
    #[must_use]
    pub fn list(&self) -> Vec<CapabilityDescriptor> {
        self.assert_owner();
        self.inner
            .entries
            .borrow()
            .values()
            .map(|entry| entry.descriptor.clone())
            .collect()
    }

    /// Validate input, run the host implementation, then validate output.
    ///
    /// Infrastructure borrows are released before host code runs. The supplied
    /// [`ExecutionContext`] is passed through unchanged.
    ///
    /// # Errors
    ///
    /// Unknown names, input contract failures, business failures, and output
    /// contract violations are distinct [`Error`] kinds.
    ///
    /// # Panics
    ///
    /// Panics when called from a thread other than the creator thread.
    pub fn invoke(
        &self,
        name: &CapabilityName,
        input: Value,
        context: &ExecutionContext,
    ) -> Result<Value, Error> {
        self.assert_owner();
        let prepared = self.prepare(name)?;
        prepared.input.validate(&input)?;
        let output = invoke_host(&prepared.capability, input, context)?;
        match prepared.output.validate(&output) {
            Ok(_) => Ok(output),
            Err(cause) => Err(Error::output_contract_violation(cause)),
        }
    }

    fn prepare(&self, name: &CapabilityName) -> Result<Prepared, Error> {
        let entries = self.inner.entries.borrow();
        let entry = entries
            .get(name)
            .ok_or_else(|| Error::unknown_capability(name.as_str()))?;
        Ok(Prepared {
            capability: Rc::clone(&entry.capability),
            input: entry.descriptor.input().clone(),
            output: entry.descriptor.output().clone(),
        })
    }

    fn assert_owner(&self) {
        assert_eq!(
            std::thread::current().id(),
            self.owner,
            "axiom Runtime is thread-affine and must be used on the thread that created it"
        );
    }

    /// Weak handle for reentry from a registered capability.
    ///
    /// Storing a strong [`Runtime`] clone inside a capability that is then
    /// registered on the same runtime creates a reference cycle. Use this
    /// handle instead.
    #[must_use]
    pub fn handle(&self) -> RuntimeHandle {
        RuntimeHandle {
            owner: self.owner,
            inner: Rc::downgrade(&self.inner),
        }
    }
}

/// Non-owning view of a [`Runtime`] used by host callbacks.
#[derive(Clone, Debug)]
pub struct RuntimeHandle {
    owner: ThreadId,
    inner: std::rc::Weak<Inner>,
}

impl RuntimeHandle {
    /// Restore a strong runtime handle.
    ///
    /// # Panics
    ///
    /// Panics if the runtime has been dropped, or on the wrong thread.
    #[must_use]
    pub fn runtime(&self) -> Runtime {
        let inner = self
            .inner
            .upgrade()
            .expect("axiom Runtime was dropped while a host callback still held a handle");
        let runtime = Runtime {
            owner: self.owner,
            inner,
        };
        runtime.assert_owner();
        runtime
    }
}

struct Prepared {
    capability: Rc<dyn Capability>,
    input: TypeContract,
    output: TypeContract,
}

fn invoke_host(
    capability: &Rc<dyn Capability>,
    input: Value,
    context: &ExecutionContext,
) -> Result<Value, Error> {
    match capability.invoke(input, context) {
        Ok(output) => Ok(output),
        Err(failure) => {
            let (message, details) = failure.into_parts();
            Err(Error::business_failure(message, details))
        }
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{BusinessFailure, CapabilityCategory, CapabilityName};
    use crate::contract::TypeContract;
    use crate::foundation::{CorrelationId, ErrorKind};
    use std::cell::Cell;
    use std::rc::Rc as StdRc;

    struct Counting {
        calls: StdRc<Cell<u32>>,
        output: Value,
    }

    impl Capability for Counting {
        fn invoke(
            &self,
            _input: Value,
            _context: &ExecutionContext,
        ) -> Result<Value, BusinessFailure> {
            self.calls.set(self.calls.get() + 1);
            Ok(self.output.clone())
        }
    }

    struct FailHost;

    impl Capability for FailHost {
        fn invoke(
            &self,
            _input: Value,
            _context: &ExecutionContext,
        ) -> Result<Value, BusinessFailure> {
            Err(BusinessFailure::new("overflow"))
        }
    }

    fn name(raw: &str) -> CapabilityName {
        CapabilityName::parse(raw).unwrap()
    }

    fn descriptor(raw: &str, input: TypeContract, output: TypeContract) -> CapabilityDescriptor {
        CapabilityDescriptor::new(
            name(raw),
            "test capability",
            CapabilityCategory::parse("test").unwrap(),
            input,
            output,
        )
        .unwrap()
    }

    fn ctx() -> ExecutionContext {
        ExecutionContext::root(CorrelationId::parse("req").unwrap())
    }

    #[test]
    fn duplicate_register_is_atomic() {
        let runtime = Runtime::new();
        runtime
            .register(
                descriptor("echo", TypeContract::Integer, TypeContract::Integer),
                Counting {
                    calls: StdRc::new(Cell::new(0)),
                    output: Value::integer(1),
                },
            )
            .unwrap();
        let err = runtime
            .register(
                descriptor("echo", TypeContract::Bool, TypeContract::Bool),
                Counting {
                    calls: StdRc::new(Cell::new(0)),
                    output: Value::bool(true),
                },
            )
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DuplicateCapability);
        assert_eq!(runtime.list().len(), 1);
        assert!(runtime.get(&name("missing")).is_none());
    }

    #[test]
    fn invalid_input_does_not_run_host() {
        let runtime = Runtime::new();
        let calls = StdRc::new(Cell::new(0));
        runtime
            .register(
                descriptor("n", TypeContract::Integer, TypeContract::Integer),
                Counting {
                    calls: StdRc::clone(&calls),
                    output: Value::integer(1),
                },
            )
            .unwrap();
        let err = runtime
            .invoke(&name("n"), Value::from("x"), &ctx())
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::TypeMismatch);
        assert_eq!(calls.get(), 0);
        runtime
            .invoke(&name("n"), Value::integer(2), &ctx())
            .unwrap();
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn unknown_invoke_differs_from_missing_discovery() {
        let runtime = Runtime::new();
        assert!(runtime.get(&name("ghost")).is_none());
        let err = runtime
            .invoke(&name("ghost"), Value::null(), &ctx())
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownCapability);
    }

    #[test]
    fn business_failure_preserves_message() {
        let runtime = Runtime::new();
        runtime
            .register(
                descriptor("fail", TypeContract::Null, TypeContract::Null),
                FailHost,
            )
            .unwrap();
        let err = runtime
            .invoke(&name("fail"), Value::null(), &ctx())
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::BusinessFailure);
        assert_eq!(err.message(), "overflow");
    }

    #[test]
    fn output_violation_is_not_success() {
        let runtime = Runtime::new();
        runtime
            .register(
                descriptor("bad", TypeContract::Null, TypeContract::Integer),
                Counting {
                    calls: StdRc::new(Cell::new(0)),
                    output: Value::from("nope"),
                },
            )
            .unwrap();
        let err = runtime
            .invoke(&name("bad"), Value::null(), &ctx())
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::OutputContractViolation);
        assert_eq!(err.path(), Some(&crate::foundation::Path::root()));
    }

    #[test]
    fn snapshots_are_sorted_and_isolated() {
        let runtime = Runtime::new();
        runtime
            .register(
                descriptor("b", TypeContract::Null, TypeContract::Null),
                FailHost,
            )
            .unwrap();
        runtime
            .register(
                descriptor("a", TypeContract::Null, TypeContract::Null),
                FailHost,
            )
            .unwrap();
        let listed = runtime.list();
        let names: Vec<_> = listed.iter().map(|item| item.name().as_str()).collect();
        assert_eq!(names, ["a", "b"]);
        let snapshot = runtime.get(&name("a")).unwrap();
        runtime
            .register(
                descriptor("c", TypeContract::Null, TypeContract::Null),
                FailHost,
            )
            .unwrap();
        assert_eq!(snapshot.name().as_str(), "a");
        assert_eq!(runtime.list().len(), 3);
    }
}
