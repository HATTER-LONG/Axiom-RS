---
name: agile-delivery
description: Deliver Axiom-RS repository changes in small, verifiable increments with adaptive decomposition, repository quality gates, bounded repair, and optional independent review. Use only when the user explicitly requests this workflow.
---

# Agile Delivery

Use the smallest delivery process that safely completes the requested change.

Repository design documents define the required Axiom semantics. `AGENTS.md`
defines engineering and quality principles. This skill defines only task
execution and coordination.

Do not create agents, worktrees, subtasks, reviews, or repeated validation
cycles unless they provide clear value for the current change.

## 1. Establish the Delivery Contract

Before implementation, identify:

- objective and observable acceptance criteria;
- owning responsibility and relevant public boundary;
- behavior that must remain unchanged;
- relevant design contract, tests, and documentation;
- dependencies or decisions that block implementation.

Express the task compactly:

```text
OBJECTIVE:
ACCEPT:
SCOPE:
PRESERVE:
DEPEND:
VERIFY:
```

Do not prescribe implementation details unless they are themselves part of the
required contract.

## 2. Shape Coherent Increments

Prefer one coherent increment when the change is localized. Decompose only when
doing so materially improves ownership clarity, context isolation, parallelism,
integration safety, verification, or delivery time.

Each increment should contain one externally meaningful behavior together with
its tests and required documentation. Do not split implementation, tests, and
documentation when they describe the same behavior. Make dependencies between
increments explicit.

## 3. Choose the Simplest Execution Model

Default to direct delivery:

```text
inspect → implement → focused verification → repository gate → deliver
```

Delegate isolated work only when it provides useful parallelism, specialized
investigation, independent review, meaningful context isolation, or risk
reduction. Parallel work must have clear ownership and no unresolved interface
dependency.

Use a worktree only when isolated Git state provides real value. Never disturb
unrelated user changes in the current working tree.

## 4. Implement Incrementally

For each coherent increment:

1. inspect the owning code, design contract, and focused tests;
2. implement the complete requested behavior;
3. add or update contract-focused tests;
4. update externally relevant documentation;
5. run the narrowest useful diagnostic checks;
6. run `cargo make fast` when the increment is coherent.

Focused commands such as `cargo test`, `cargo clippy`, or checker-specific tests
help with diagnosis but do not replace repository-defined delivery gates.

## 5. Integrate in Dependency Order

When multiple increments exist, integrate them in dependency order. Resolve
simple textual conflicts directly.

Do not mechanically resolve conflicts involving semantics, ownership, lifetime,
public API, state behavior, concurrency, or error contracts. Re-evaluate them
against the task contract and repository design.

After a substantial integrated change, run the strongest relevant gate:

```text
cargo make fast
cargo make full
cargo make hardening
```

A stronger successful gate covers weaker checks for the same unchanged state.
Any relevant semantic change invalidates downstream validation performed before
it.

## 6. Repair by Root Cause

When validation fails:

```text
failure → inspect diagnostic → identify root cause → repair owner → rerun check
```

Repair localized failures directly when ownership is clear. Isolate or delegate
repair only when it spans responsibilities, has unclear ownership, needs
specialized investigation, or materially benefits from independent context.

Give a clearly identified root cause one bounded repair attempt before
escalating the same failure. If it remains, report:

```text
failing check
root cause
repair attempted
rerun result
repository state
```

A newly revealed, different failure may enter its own repair cycle. Never bypass
or weaken a gate.

## 7. Use Hardening Proportionally

Use `cargo make hardening` when a change meaningfully affects unsafe code,
ownership, lifetime, concurrency, cancellation, state transitions, serialization
or dynamic boundaries, or panic and failure isolation.

Do not run expensive validation mechanically when it provides no additional
confidence for the change.

## 8. Use Independent Review Proportionally

Independent review is optional. Use it when separate reasoning materially
reduces risk, especially for public API, runtime semantics, unsafe code,
ownership and lifetime, concurrency, dynamic data boundaries, persistence or
serialization contracts, security-sensitive behavior, or cross-responsibility
architecture.

Review only a coherent, validated candidate. Request actionable findings about
correctness, contract, ownership, architecture, edge cases, and tests rather
than a general summary. Revalidate after semantic fixes.

## 9. Keep Handoffs Compact

Communicate only what another contributor needs to continue:

```text
objective:
acceptance:
scope:
preserve:
dependencies:
verification:
issues:
```

Do not transmit long reasoning logs, duplicated `AGENTS.md` content, unchanged
repository background, or speculative implementation ideas. The receiving
contributor should be able to continue from the contract and repository itself.

## 10. Complete Delivery

Before delivery:

- confirm implementation matches acceptance criteria;
- confirm tests protect the requested behavior;
- confirm documentation matches observable behavior;
- confirm unrelated behavior remains intact;
- run `cargo make full` on the final candidate;
- add `cargo make hardening` when relevant.

Report only delivered behavior, public contract impact, validation actually
executed and its result, and unresolved issues. Do not claim completion, review,
validation, or repair success unless it actually occurred.
