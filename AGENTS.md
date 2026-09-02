# Agent Guide

Build Axiom-RS in small, coherent, verifiable increments.

Repository design and requirement documents define **what Axiom means**. This
guide defines **how Axiom-RS should be engineered in Rust**.

Prefer idiomatic Rust solutions that satisfy the required semantics. Do not
infer architecture, abstractions, or extension points that are not justified by
the current requirements.

Examples clarify intent only; they are not implementation templates.

## 1. Scope

- Implement only what the current task requires.
- Prefer the smallest correct change over broad refactoring.
- Preserve unrelated behavior unless explicitly changed.
- Before editing, identify:
  - the owning responsibility;
  - the relevant public boundary;
  - externally visible behavior;
  - behavior that must remain unchanged;
  - relevant tests and documentation.
- Avoid speculative abstractions, unrelated cleanup, and future-oriented
  extension points.

A task should normally produce one coherent behavior change together with its
tests and required documentation.

## 2. Design

Prefer **thin interfaces and deep implementations**.

- Keep state, coordination, validation, policy, representation, and
  synchronization inside the responsibility that owns them.
- Prefer changing internal implementation over expanding public API.
- Add public types, traits, methods, options, callbacks, generic parameters, or
  extension points only when required by current callers.
- Keep dependencies directional and avoid cycles.
- Keep ownership, lifetime, errors, ordering, side effects, and concurrency
  explicit at boundaries.
- Prefer composition over global coordination objects.
- Prefer value semantics where they naturally fit the domain.
- Use traits for meaningful behavioral abstraction, not merely to hide concrete
  types.
- Avoid vague catch-all areas such as `utils`, `common`, `manager`, or generic
  service containers.
- Do not expose storage details, executor details, synchronization primitives,
  third-party implementation types, or mutable internals unless they belong to
  the intended public contract.

Higher-level adapters may translate Axiom semantics into another representation,
but they must not redefine underlying capability, resource, task, value, error,
or execution behavior.

## 3. Rust

Choose the simplest idiomatic Rust representation that preserves the required
semantics.

- Prefer stable Rust and the standard library first.
- Use the type system to express invariants when it improves correctness without
  making the public API unnecessarily complex.
- Keep the crate safe by default.
- Isolate necessary `unsafe` behind a small safe abstraction, document every
  safety invariant, and test the observable behavior around it.
- Prefer explicit ownership over hidden shared ownership.
- Avoid unnecessary allocation, cloning, boxing, dynamic dispatch, and interior
  mutability; use them when they provide a clear ownership, API, or runtime
  benefit.
- Do not add `Send`, `Sync`, or `'static` bounds speculatively.
- Do not introduce async APIs unless asynchronous semantics belong to the
  requirement.
- Keep platform-specific behavior behind narrow portability boundaries.
- Avoid hard-coded paths, separators, shells, environment assumptions, or
  deployment details.
- Use feature flags only for genuinely optional behavior.

## 4. Runtime Boundaries

Axiom-RS is runtime infrastructure, so make the following explicit when
relevant:

- who owns state;
- how long references or handles remain valid;
- which operations may overlap;
- what happens during shutdown or removal;
- whether callbacks may re-enter the runtime;
- whether observations are snapshots or live views;
- which errors are expected runtime failures;
- which conditions represent programming defects.

Infrastructure synchronization should not surround arbitrary user code unless
the contract explicitly requires serialization. Prefer:

```text
resolve required state
        ↓
leave infrastructure synchronization
        ↓
execute user behavior
```

This reduces coupling, deadlock risk, and hidden serialization.

## 5. Errors and Validation

Failures are part of the public behavior.

- Validate at the boundary that owns the rule.
- Do not duplicate validation across layers.
- Preserve structured errors rather than replacing them with generic failures.
- Preserve useful location or path information for structured input.
- Reject invalid states before they create partially valid runtime objects.
- Do not silently coerce invalid input unless coercion is an explicit
  requirement.
- Do not silently discard failures.

A higher-level coordinator should validate only its own input structure and
leave domain semantics to the owning responsibility.

## 6. Implementation

- Implement the smallest complete solution satisfying the contract.
- Keep control flow and state transitions explicit.
- Reuse existing abstractions when they already fit.
- Keep changes localized to the owning responsibility.
- Avoid mixing behavior changes with unrelated renaming or restructuring.
- Do not introduce a general framework for one concrete use case.
- Do not make internal representation observable accidentally.
- Prefer deterministic behavior when ordering is externally visible.

If the current design does not support requested behavior cleanly, change the
owning abstraction rather than layering workarounds around it.

## 7. Tests

Tests should protect contracts rather than implementations.

- New behavior requires tests.
- Bug fixes require regression tests when practical.
- Prefer externally meaningful assertions.
- Cover important success and failure boundaries.
- Cover ownership, lifetime, state transition, and concurrency behavior when
  relevant.
- Use unit tests for focused contracts and integration tests for public
  cross-boundary behavior.
- Keep tests deterministic and independent.
- Do not assert private call counts or internal layout unless that detail is
  itself required behavior.
- Never weaken or remove tests merely to make a change pass.

Prefer testing:

```text
Given invalid structured input,
the public operation returns the expected structured error at the correct path.
```

rather than testing which private helper detected it.

## 8. Documentation

Keep documentation synchronized with observable behavior.

- Repository design and requirement documents are the source of truth for Axiom
  product and runtime semantics.
- Use crate-level and module-level documentation for purpose and boundaries.
- Document public contracts with `///`.
- Document ownership, lifetime, ordering, thread-safety, side effects, and
  failure behavior when meaningful.
- Add `# Errors`, `# Panics`, and `# Safety` sections where appropriate.
- Include examples when they clarify intended usage.
- Explain constraints and invariants rather than restating code.

When code and design documentation disagree, do not silently choose one.
Resolve the inconsistency as part of the task or report it.

## 9. Quality

Use repository-defined workflows:

```text
cargo make fast       focused development validation
cargo make full       complete delivery validation
cargo make hardening  deeper runtime and test-strength validation
```

- Run `cargo make fast` after a coherent implementation increment.
- Run `cargo make full` on the final delivery candidate.
- Run `cargo make hardening` when deeper validation provides meaningful
  confidence for the affected runtime behavior.
- Use focused `cargo test`, `cargo clippy`, or checker tests for diagnosis.
- A stronger successful gate covers weaker checks for the same unchanged state.
- Diagnose and fix root causes instead of repeatedly rerunning failing gates.

Never make a gate pass by disabling or skipping tests, weakening thresholds,
adding unjustified exclusions, suppressing valid diagnostics, bypassing
repository checks, or changing required semantics.

## 10. Efficiency and Workspace

- Inspect the current working state before editing.
- Preserve unrelated user changes and untracked files.
- Prefer targeted symbol, caller, and test searches over broad repository reads.
- Reuse established context rather than rediscovering it.
- Do not create agents, worktrees, reviews, or validation passes unless they
  provide meaningful isolation, parallelism, independence, or risk reduction.
- Do not treat generated files, dependency directories, or caches as project
  source unless explicitly required.
- Keep reports concise and factual.

## 11. Done

A task is complete when:

- requested behavior is implemented in the correct ownership boundary;
- observable contracts match the requirements;
- relevant tests protect the behavior;
- required documentation matches the implementation;
- unrelated behavior remains intact;
- required validation passes;
- no known unresolved issue affects the requested behavior.

Never claim that a test, gate, review, or validation passed unless it actually
ran and passed.
