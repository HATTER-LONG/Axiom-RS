# Axiom-RS

## Planning

- [Overall development plan](docs/development-plan.md)
- [Phase 1 foundation tasks](docs/phase-1-tasks.md)

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
