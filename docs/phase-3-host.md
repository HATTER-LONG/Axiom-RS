# Phase 3 host freeze

This is a **reference host**, not a production application. Real host selection
remains open. Decisions that depend on a production thread model or domain state
are recorded here as reference-host conclusions only.

## Host

`crates/lab-inspect` is an engineering-inspection calculator: summary statistics,
axis-aligned box measures, dilution arithmetic, and temperature conversion.

No Resource or Task is required. All four capabilities are synchronous functions
over owned values plus optional in-memory counters in tests.

## Capabilities

| Name | Input | Output | Business failure | Threads |
| --- | --- | --- | --- | --- |
| `stats.summarize` | `{ samples: [integer] }` | `{ count, min, max, mean }` | empty `samples` | caller thread |
| `geom.axis_aligned_box` | `{ size: { x, y, z }, unit }` nested | `{ volume, surface, unit }` | none beyond contract | caller thread |
| `chem.dilute` | `{ stock_mM, target_mM, volume_mL }` | `{ diluent_mL, aliquot_mL }` | `target_mM > stock_mM` | caller thread |
| `temp.convert` | `{ value, from, to }` | `{ value, unit }` | Kelvin result `< 0` | caller thread |

`geom.axis_aligned_box` is the nested-input case. `stats.summarize` is real
aggregation. `chem.dilute` and `temp.convert` are predictable business failures.

## Parameter contracts

Shape, field descriptions, enum membership, and numeric ranges live in
`TypeContract` / `FieldContract`. The host converts `Value` to local numbers
after Runtime validation and applies domain rules only.

## External entry

JSON/stdio Adapter (`crates/axiom-stdio`). One NDJSON request object per line on
stdin; one NDJSON response per line on stdout. Diagnostics go to stderr only.

## Execution model

Thread-affine `Runtime` handle. Clone shares the table on the creating thread.
Capabilities are not `Send`/`Sync`. Cross-thread use panics as a programming
defect. Reentry of discover/invoke is supported because infrastructure borrows
are dropped before host code runs.

## Parameter maintenance

Adding a field (for example `geom.axis_aligned_box` `size.w`) requires:

1. `FieldContract` in `box_input`;
2. reading it in `box_metrics`;
3. a host or e2e test that fails if the contract and conversion disagree.

No business rule lives in Command or the Adapter. Shared conversion helpers stay
private to this crate.

## Remaining production-host acceptance

P3-012 item 5 (real application usage) is **not** claimed. The next phase should
choose Resource or Task only after a real host exists; until then the state
extension is recorded as not required by this reference calculator.

1. Send `{"v":1,"cmd":"list"}`.
2. Send `{"v":1,"cmd":"get","name":"geom.axis_aligned_box"}` and read field
   names, nested `size`, `unit` enum, and ranges.
3. Send an invoke with a nested type error; read `kind`, `path`, and details.
4. Correct the payload and invoke successfully.
