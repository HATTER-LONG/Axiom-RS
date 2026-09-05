# Axiom-RS

## Planning

- [Axiom overall design](docs/Axiom-design.md)
- [Overall development plan](docs/development-plan.md)
- [Phase 1 foundation tasks](docs/phase-1-tasks.md)
- [Phase 2 capability and discovery tasks](docs/phase-2-tasks.md)
- [Phase 3 foundation fixes and usable MVP tasks](docs/phase-3-tasks.md)

## Current status

Phase 1 primitives and Phase 2 capability metadata registration/discovery are implemented.
Phase 3 has not started. A confirmed object-contract mutation bypass remains to be fixed;
passing quality gates does not establish that this invariant holds.

The revised [development plan](docs/development-plan.md) starts Phase 3 with that fix and
architecture-checker regression coverage, then delivers a real host integration, synchronous
invocation, a minimal command boundary, and one external adapter. These are planned work,
not currently available features.

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
