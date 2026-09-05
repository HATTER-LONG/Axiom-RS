---
name: agile-delivery
description: Organize Axiom-RS work into coherent delivery increments when the user explicitly requests this workflow.
---

# Agile Delivery

Use this workflow for the requested scope. Project contracts live in `AGENTS.md`
and the relevant design/task documents; verification choices live in
`README.md#development-and-verification`.

## Shape the work

For a localized change, deliver it directly. For a larger task, identify observable
acceptance criteria, ownership, dependencies, and a useful verification approach.
Keep this compact; no fixed planning or handoff template is required.

Split work where parts can be accepted independently or where integration risk
warrants it. Keep implementation, contract tests, and associated documentation
in the same increment. Task numbers need not map one-to-one to commits.

Use agents or worktrees only when authorized and they offer concrete parallelism,
independence, or isolation. A handoff needs the scope, acceptance criteria,
dependencies, and current verification or blocker, not duplicated repository rules.

## Integrate and finish

Integrate in dependency order. Resolve semantic conflicts against the owning
contract. Optional review may help with a design decision or a coherent change;
it is not a prerequisite for every edit or something that must await every gate.

Choose checks from the repository verification guide. Revalidate affected behavior
after fixes; an unchanged passing candidate does not need another validation round
at each handoff or commit.

Continue diagnosing and repairing in-scope failures while there is a useful next
step. Ask for input when a decision or external condition actually blocks progress,
not after an arbitrary number of attempts. Report unresolved failures accurately.

Deliver when the requested acceptance criteria and applicable checks are satisfied.
A missing production-host acceptance blocks that milestone, not unrelated core work.
