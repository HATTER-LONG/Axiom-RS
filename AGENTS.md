# Axiom-RS Agent Guide

Axiom-RS is Rust runtime infrastructure for exposing host capabilities through
native and dynamic entry points. Prefer a small, idiomatic implementation of the
current contract over speculative frameworks or extension points.

## Relevant context

- For product/runtime semantics, consult the relevant sections of
  [Axiom design](docs/Axiom-design.md).
- For delivery scope and milestone acceptance, use the
  [development plan](docs/development-plan.md) and the relevant phase task.
  Historical baselines and task-specific restrictions are not standing rules.
- For setup and verification selection, use
  [README](README.md#development-and-verification). `Makefile.toml` implements
  those commands; phase documents identify behaviors to test, not extra gate cycles.
- Use the [agile-delivery skill](.agents/skills/agile-delivery/SKILL.md) only when
  explicitly requested. Ordinary changes do not need a delivery ceremony.

Read the context needed for the change; a full repository or document survey is
not required before each edit. If a relevant implementation and requirement
disagree, resolve that inconsistency in scope or report it.

## Engineering boundaries

- Keep validation, state, ordering, and representation in their owning module.
  Add public API only for current callers; higher-level adapters translate Axiom
  semantics rather than redefining them. `architecture.toml` records layer rules.
- Validate untrusted inputs at the boundary owning the rule. Preserve structured
  errors and useful paths; do not silently coerce invalid input. Do not duplicate
  validation across layers or add runtime checks solely to repeat type guarantees.
- Keep ownership, handle lifetime, thread affinity, re-entry, and failure behavior
  explicit where they affect callers. Release infrastructure locks and mutable
  borrows before executing host code unless serialization is part of the contract.
- Prefer stable, safe Rust. Isolate necessary unsafe code behind a documented safe
  boundary. Add async APIs, shared ownership, trait abstractions, or `Send`/`Sync`
  bounds only when the selected execution model and current callers need them.
- Preserve the project's stated platform support. Use portable Rust APIs for paths
  and processes; isolate necessary OS-specific code with narrow `cfg` boundaries.
  Scripts and tasks should avoid assuming a shell, path separator, or architecture.
  Label platform-specific commands and distinguish untested platforms from tested
  support. Cross-platform checks are needed when the change affects portability,
  not as an additional gate for every edit.
- Test new observable behavior and practical bug regressions at the appropriate
  public boundary. Assert contracts, not private layout or call counts. Type-level
  guarantees can be protected by compile-fail tests; do not use unsafe code merely
  to manufacture an impossible safe-caller misuse.
- Document public contracts with rustdoc, including meaningful ownership and
  failure constraints. Update design or usage docs when the observable contract
  changes, rather than documenting internal implementation steps.

## Verification and delivery

Follow the README's change-based verification policy. `full` covers `fast`'s
checks, using coverage instrumentation for target tests. Hardening tools are
selected individually by risk and do not replace either flow.

Diagnose failures and rerun affected checks after an in-scope fix. Do not disable
tests or hide valid diagnostics to obtain a pass. Changes to thresholds or gate
policy need an explicit rationale and review as workflow changes; they are not a
workaround for a failing implementation.

Keep changes and any requested commits coherent and scoped. Task numbers do not
dictate commit count. Preserve unrelated modifications; a clean whole worktree
is not a delivery requirement. Report delivered behavior, actual validation, and
remaining blockers. Product milestone acceptance is distinct from completion of
an individual task.
