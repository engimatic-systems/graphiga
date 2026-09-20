# Query spike comparison results

All three arms implemented the same schema-resident named queries and passed
the shared black-box suite. Each completed one independent simplicity review.
No winner has been selected and no implementation was merged to `main`.

Common implementation baseline: `6c59a754c175f0dd6b2c818b6c61114036999620`.
The dataset, expected results, shared acceptance runner and query meanings
remained unchanged in all implementation branches. Nemo alone translated its
query template to the engine's native syntax, as authorized.

## Measured implementation costs

Production Rust counts are net additions relative to the common baseline,
including integration changes but excluding tests, docs and dependency source.
They are maintenance-surface measurements, not quality or performance scores.

| Arm | Net production Rust | Added dependencies | Toolchain |
| --- | ---: | --- | --- |
| Predicates | 775 lines | None | Existing stable pin |
| Nemo | 245 lines | Nemo + Tokio; locked packages 53 -> 345 | Branch-local pinned nightly |
| Datafrog | 1,062 lines | Datafrog 2.0.1; no transitive additions | Existing stable pin |

- **Predicates:** concise query text and direct evaluation, with two deliberately
  bounded graph operators. Owns a tokenizer/parser and schema-bound expressions,
  but no general rule planner, recursion engine or stratification machinery.
  Bare-token syntax limits expressible field names and values. It does not offer
  user-defined recursive relations.
- **Nemo:** smallest owned adapter (209-line query module), using the existing
  parser, native typed facts and reasoning engine. This delegates substantially
  more machinery, with substantially more dependencies and a nightly requirement.
  Its final four-component development toolchain closure measured 912.4 MiB,
  reduced from the reviewer's 2.0 GiB complete-toolchain measurement at the same
  pin. That is a development-toolchain measurement, not application runtime size.
- **Datafrog:** small dependency footprint, but owns the full bounded textual
  language, binding, stratification and dynamic rule planning. Its 1,028-line
  query module is the principal cost; Datafrog genuinely performs relational
  evaluation rather than serving as a cosmetic dependency.

The successful tiny workload does not establish large-store performance or
production security/durability. These are trusted-schema, read-only query spikes.
No release-size, throughput or memory benchmark comparison was performed.

## Validation and exact revisions

Every final candidate passed formatting, its full Rust tests, warnings-denied
all-target Clippy, build, and `query-spikes/verify.py` for its declaration dialect.
The shared suite exercises local filtering, dependency readiness, transitive
parent traversal, mutations, cycles, renamed/changed declarations, empty stores,
errors, output shape/order and read-only behavior.

| Arm / branch | Final tested and pushed | Independently reviewed | Disposition |
| --- | --- | --- | --- |
| Predicates / `spike/query-predicates` | `3f92f04084468b102078e826c21b1819b0ef929e` | `466e890c1a2bc61878184bd7a63a484e15223f5e` | Document field-name limits; consolidate adjacency logic. Repairs tested, **not independently re-reviewed**. |
| Nemo / `spike/query-datalog` | `c708cfdf013ca53b97d29e8badd478a4af150110` | `768d3b6b5597006bb177adeb05b6abbdceabab89` | Narrow pinned toolchain components. Repair tested, **not independently re-reviewed**. |
| Datafrog / `spike/query-datafrog` | `17ad26ca871d4b3be7f39cb9a0a1bb5b7ce26fef` | Same revision | No actionable findings; no repairs. |

Reviews were simplicity-focused and reported material correctness issues if
encountered, not exhaustive correctness/security/performance audits. No material
query correctness defect was reported. The primary verified clean final working
trees, common-input preservation, production line counts and published revisions;
it did not implement the engines or perform another source review.

## Detailed reports and system of record

- [Predicate report](https://github.com/engimatic-systems/graphiga/blob/3f92f04084468b102078e826c21b1819b0ef929e/query-spikes/PREDICATES-RESULTS.md)
- [Nemo report](https://github.com/engimatic-systems/graphiga/blob/c708cfdf013ca53b97d29e8badd478a4af150110/query-spikes/NEMO-REPORT.md)
- [Datafrog implementation report](https://github.com/engimatic-systems/graphiga/blob/17ad26ca871d4b3be7f39cb9a0a1bb5b7ce26fef/query-spikes/DATAFROG_RESULTS.md)
  captures the candidate before review; its pending-review paragraph is superseded
  by the completed review disposition above and the ticket evidence.

Sys tickets: comparison `sys-2dxc`, predicates `sys-w44b`, Nemo `sys-9tqt`,
Datafrog `sys-2fb7`. Review/evidence details remain in those tickets and their
linked artifacts. Implementation tickets remain open for operator acceptance;
completion of the experiment does not select an engine or authorize a merge.
