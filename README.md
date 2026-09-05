# Axiom-RS

## Planning

- [Axiom overall design](docs/Axiom-design.md)
- [Overall development plan](docs/development-plan.md)
- [Phase 1 foundation tasks](docs/phase-1-tasks.md)
- [Phase 2 capability and discovery tasks](docs/phase-2-tasks.md)
- [Phase 3 foundation fixes and usable MVP tasks](docs/phase-3-tasks.md)

## Current status

Phase 1 primitives, Phase 2 metadata registry, and Phase 3 core MVP are on
`feat/phase-3-mvp`: sealed object contracts, architecture-checker qualified-path
coverage, thread-affine `Runtime`, Command list/get/invoke, JSON/stdio adapter,
and a **reference** host (`lab-inspect`). A production host has not been
selected; that remaining acceptance is recorded in
[phase 3 tasks](docs/phase-3-tasks.md) and [host freeze](docs/phase-3-host.md).

## Reference host (stdio)

```bash
cargo run -p lab-inspect
```

Send NDJSON on stdin. Integers and floats are tagged so they are not mixed.
Objects that use `$i`, `$f`, or `$o` as field names are wrapped as `{"$o":{...}}`.
Request lines are at most 65536 bytes; nesting is at most 32 Axiom Value levels.

```json
{"v":{"$i":"1"},"cmd":"list"}
{"v":{"$i":"1"},"cmd":"get","name":"geom.axis_aligned_box"}
{"v":{"$i":"1"},"cmd":"invoke","name":"stats.summarize","input":{"samples":[{"$i":"2"},{"$i":"4"}]},"correlation_id":"req-1"}
```

Responses go to stdout. Adapter diagnostics go to stderr. Protocol errors use
the same tagged integers as successful responses. Error paths are
`{"segments":[...],"display":"..."}` or `null` when absent.

## Setup

Use Rust through rustup; `rust-toolchain.toml` selects stable and its components.
Install cargo-make and [uv](https://docs.astral.sh/uv/getting-started/installation/).
The checkers need Python 3.11+; tasks invoke it through `uv run --quiet python`.

```bash
cargo install cargo-make
```

Coverage uses `llvm-tools-preview` from the toolchain file and cargo-llvm-cov
(installed by the coverage task if needed). Nightly and Lizard are optional;
install them only for the corresponding diagnostics below.

## Development and verification

This section owns workflow selection. Phase documents define acceptance behavior;
they do not require extra validation rounds beyond this policy.

| Change | Verification |
| --- | --- |
| Prose or prompt instructions only | Review the diff, links, scope, and consistency; validate skill metadata when changed. No Rust gate is required. |
| Local code increment | Run affected tests. Use `cargo make fast` for broader development feedback when useful. |
| Final Rust code candidate | Run `cargo make full` once. It covers fast's checks using instrumented target tests. |
| Verification scripts or workflow configuration | Run affected checker tests and inspect task expansion; exercise changed commands. Run `full` when its test orchestration changes. |
| A concrete memory-safety or test-strength risk | Select a relevant hardening tool and test scope; ordinary protocol or state changes do not automatically require every tool. |

```bash
cargo make fast   # format, architecture, checker tests, target tests, doctests
cargo make full   # also Clippy, API docs, lines/regions coverage >= 90%
```

`full` runs target tests through coverage instead of running the same suite twice.
Doctests, including compile-fail cases, run separately. A successful full run
covers fast for the same candidate; there is no need to run fast first. Reuse
results while their relevant inputs are unchanged. After a fix, rerun affected
checks; broaden validation when the change or a failure warrants it.

The 90% line and region thresholds remain delivery checks, not substitutes for
contract tests. Do not hide defects by changing tests, diagnostics, or exclusions.
A justified workflow or threshold change is reviewed on its own merits; ordinary
percentage fluctuation above the threshold is not a separate failure.

Useful focused commands:

```bash
cargo make compile-check
cargo make architecture-check
cargo make checker-tests
cargo make test-docs
cargo make coverage-check
```

### Optional diagnostics

There is no aggregate hardening gate: Miri, ASan, and mutation testing address
different risks. Select a tool for a stated concern and use a focused package or
test command when the workspace-wide task is unnecessarily broad. Unsupported
platforms or unexecuted tests are reported as not run, never as passes.

- `cargo make miri-check`: interpreted tests for applicable memory/aliasing risks.
  Requires `rustup +nightly component add miri`. Process-based tests may need
  ordinary integration testing instead; choose compatible tests explicitly.
- `cargo make asan-linux`: native **x86_64 Linux GNU** only.
- `cargo make asan-windows`: native **x86_64 Windows MSVC** only; put
  `clang_rt.asan_dynamic-x86_64.dll` on `PATH`.
  Both ASan tasks require `rustup +nightly component add rust-src`.
  Other architectures/platforms need their own supported command if ASan is needed;
  these tasks do not auto-detect or silently skip platforms.
- `cargo make mutation-check`: investigate whether contract tests catch meaningful
  changes. Reviewed equivalent/non-contract exclusions live in `.cargo/mutants.toml`.
- `cargo make complexity-report`: optional Lizard diagnostics (`pip install lizard`).
  Counts guide review and do not block delivery or require splitting clear code.

### Commits and cleanup

When committing is requested, group changes by a coherent, reviewable outcome.
Keep its tests and documentation together; neither task count nor behavior count
sets the number of commits. Inspect the staged diff and describe why the change
exists and the relevant verification. Preserve unrelated edits and untracked files;
cleanup is limited to artifacts created for the task. A clean whole worktree is
not a prerequisite for delivery.
