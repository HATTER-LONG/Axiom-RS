# Phase 3 host freeze

This is a **reference host**, not a production application. Real host selection
remains open. Decisions that depend on a production thread model or domain state
are recorded here as reference-host conclusions only.

Reference **function code** for this calculator is in-tree. That is not the same
as phase-3 acceptance: protocol-boundary, cross-entry, and maintenance evidence
for the reference host are listed below; **real application usage is still
open**.

## Host

`crates/lab-inspect` is an engineering-inspection calculator: summary statistics,
axis-aligned box measures, dilution arithmetic, and temperature conversion.

No Resource or Task is required. All four capabilities are synchronous functions
over owned values plus optional in-memory counters in tests.

## Capabilities

| Name | Input | Output | Business failure | Threads |
| --- | --- | --- | --- | --- |
| `stats.summarize` | `{ samples: [integer] }` | `{ count, min, max, mean }` | empty `samples`; results that are not finite | caller thread |
| `geom.axis_aligned_box` | `{ size: { x, y, z }, unit }` nested | `{ volume, surface, unit }` | non-finite volume or surface | caller thread |
| `chem.dilute` | `{ stock_mM, target_mM, volume_mL }` | `{ diluent_mL, aliquot_mL }` | `target_mM > stock_mM`; non-finite volumes | caller thread |
| `temp.convert` | `{ value, from, to }` | `{ value, unit }` | Kelvin result `< 0`; non-finite conversion | caller thread |

`geom.axis_aligned_box` is the nested-input case. `stats.summarize` is real
aggregation. `chem.dilute` and `temp.convert` are predictable business failures.

Business arithmetic is private and strongly typed in `crates/lab-inspect/src/business.rs`.
`Value` conversion stays in `host.rs` at the `Capability` boundary. Contract
range and enum checks are not repeated there.

## Parameter contracts

Shape, field descriptions, enum membership, and numeric ranges live in
`TypeContract` / `FieldContract`. After Runtime validation the host maps `Value`
to local numbers and applies domain rules only.

## External entry

JSON/stdio Adapter (`crates/axiom-stdio`). One NDJSON request object per line on
stdin; one NDJSON response per line on stdout. Diagnostics go to stderr only.

Integers and floats are tagged (`{"$i":"..."}`, `{"$f":"..."}`). Objects whose
keys include `$i`, `$f`, or `$o` are wrapped as `{"$o":{...}}`. Untagged JSON
numbers are rejected. [`MAX_FRAME_BYTES`](../crates/axiom-stdio/src/json.rs) is
65 536 bytes per line, enforced while reading; an oversized line is rejected and
the rest of that line is discarded so the next request can be served.
[`MAX_DEPTH`](../crates/axiom-stdio/src/json.rs) is 32 Axiom `Value` levels.
Tag wrappers are not extra Value levels.

Error `path` is `null` when the error has no path. A present path is
`{"segments":[...],"display":"..."}`. Envelope failures are relative to the
command request. Invoke input and output failures are relative to those values,
not prefixed with `input`.

## Execution model

Thread-affine `Runtime` handle. Clone shares the table on the creating thread.
Capabilities are not `Send`/`Sync`. Cross-thread use panics as a programming
defect. Reentry of discover/invoke is supported because infrastructure borrows
are dropped before host code runs.

## Parameter maintenance

Recorded verification (2026-09-05, follow-up):

1. A current `geom.axis_aligned_box` payload is validated against the live
   contract, then converted by `box_from_value`.
2. `omitted_size_w_changes_observable_volume` validates a payload with extra
   `size.w=10` against a **test-only** contract that requires `w`. Live
   conversion still ignores `w` and yields volume `1`; a conversion that
   multiplies `w` yields volume `10`. The live production contract rejects `w`
   as an unknown field, so the product API was not extended.

That is fail-before-fix evidence for “add a factor and forget to read it.”
It is not a production parameter change.

Adding a field still requires:

1. `FieldContract` in `box_input`;
2. reading it in `box_from_value` / the typed box function;
3. updating `box_contract_fields_match_conversion`.

No business rule lives in Command or the Adapter.

### Access boundary and maintenance cost

Approximate private-line counts after the typed split (business arithmetic /
descriptor / Value conversion). Counts exclude tests.

| Capability | Business | Contract / descriptor | Value conversion |
| --- | ---: | ---: | ---: |
| `stats.summarize` | ~25 (`summarize_samples`) | ~20 (`stats_descriptor`) | ~15 (`samples_from_value`, `summary_to_value`) |
| `geom.axis_aligned_box` | ~15 (`axis_aligned_box`) | ~40 (`box_descriptor`, `box_input`) | ~20 (`box_from_value`, `box_to_value`) |
| `chem.dilute` | ~20 (`dilute`) | ~20 (`dilute_descriptor`) | ~15 (field reads, `dilution_to_value`) |
| `temp.convert` | ~25 (`convert_temperature`) | ~20 (`temp_descriptor`) | ~20 (scale mapping, output object) |

Shared host helpers (object/float extractors, descriptor builders) are not
counted twice. Nested `size` is the main extra contract cost. No public
conversion framework.

## Remaining production-host acceptance

P3-012 item 5 (real application usage) is **not** claimed. The next phase should
choose Resource or Task only after a real host exists; until then the state
extension is recorded as not required by this reference calculator.

Reference-host discovery/correction steps (tagged JSON):

1. Send `{"v":{"$i":"1"},"cmd":"list"}`.
2. Send `{"v":{"$i":"1"},"cmd":"get","name":"geom.axis_aligned_box"}` and read
   field names, nested `size`, `unit` enum, and ranges.
3. Send an invoke with a nested type error; read `kind`, structured `path`, and
   details.
4. Correct the payload and invoke successfully.

These steps are automated for the reference binary in
`crates/lab-inspect/tests/process.rs`. They do not replace a production host.
