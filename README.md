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

Send NDJSON on stdin. Integers and floats are tagged so they are not mixed:

```json
{"v":{"$i":"1"},"cmd":"list"}
{"v":{"$i":"1"},"cmd":"get","name":"geom.axis_aligned_box"}
{"v":{"$i":"1"},"cmd":"invoke","name":"stats.summarize","input":{"samples":[{"$i":"2"},{"$i":"4"}]},"correlation_id":"req-1"}
```

Responses go to stdout. Adapter diagnostics go to stderr.

## Setup

```bash
cargo install cargo-make
rustup component add llvm-tools-preview
rustup +nightly component add miri rust-src
pip install lizard
```

Need **Python 3.11+**. `cargo-make` uses `python3` on Unix and `py -3` on Windows.

On Windows, add `clang_rt.asan_dynamic-x86_64.dll` to `PATH` before AddressSanitizer.

## Tests

```bash
cargo make fast         # architecture + coverage (lines / regions 90%)
cargo make full         # format, clippy, complexity, docs, architecture, coverage
cargo make hardening    # miri, asan, mutants
```

Single gates:

```bash
cargo make architecture-check
cargo make coverage-check
```

Checker unit tests:

```bash
python3 -m unittest discover -s scripts
# Windows:
py -3 -m unittest discover -s scripts
```
