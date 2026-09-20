# Nemo query spike report

This report covers `spike/query-datalog` relative to common base
`6c59a754c175f0dd6b2c818b6c61114036999620`. It reports one comparison arm;
it does not select a winner or propose a production migration.

## 1. Authoring experience and expressive limits

The schema stores complete Nemo programs. The branch-specific
[`datalog.toml`](datalog.toml) uses Nemo's native syntax:

```nemo
active(?N) :- attr(?N, "status", "open").
unresolved(?N) :- edge(?N, "deps", ?D), ~closed(?D).
result(?N) :- active(?N), ~unresolved(?N).
```

Variables carry a `?` prefix, `~` negates an atom, repeated rules form a union,
and recursive rules use the same syntax as non-recursive rules. Nemo owns
parsing, validation, planning, positive fixpoint evaluation, and stratified
negation. Graphiga does not translate another rule language into Nemo.

Every named program must define unary `result(?NodeId)`. The read-only base
relations are:

- `node(Id, Type, Title)` for every document;
- `attr(Id, AttributeName, Value)` for non-reference scalars and each string-set
  member; Boolean and integer values remain native Nemo values rather than
  becoming strings; and
- `edge(SourceId, AttributeName, TargetId)` for singular and set references,
  with both directions projected for undirected attributes.

The adapter rejects query definitions of those base relations, wrong base/result
arities, and Nemo import/export directives. Imports are rejected before engine
initialization, so a schema query cannot trigger file or network input. Other
Nemo language features are not comprehensively sandboxed; schema files are a
trusted input in this spike. There are no query parameters, effects, generated
Rust, or persistent/materialized query facts.

Graphiga constructs native Nemo `Fact` values after parsing the textual query.
This avoids a second fact-file syntax and preserves arbitrary document strings,
Booleans, and signed 64-bit integers. Query results must be strings naming
existing nodes; duplicate rows collapse and presentation remains ascending by
node ID.

## 2. Implementation, dependencies, and integration cost

The integration has one engine-specific module, `src/query.rs` (209 lines at the
initial tested candidate). Relative to the common base, the initial candidate
adds 249 production Rust lines and 105 integration-test lines, with four removed
production lines. The remaining integration points are intentionally small:

- `schema` parses the optional root `[queries]` table;
- `main` adds `query NAME [--json]` and a current-thread Tokio runtime;
- `graph` resolves the named declaration and reuses the existing compact record
  rendering; and
- `query` checks the public relation contract, projects facts, invokes Nemo, and
  extracts node IDs.

There is no engine trait, alternate backend, bespoke Datalog lexer/parser,
planner, dependency graph, join implementation, or fixpoint loop. Contract
checks inspect Nemo's parsed program model rather than reparsing source text.

Direct additions are:

- Nemo `v0.10.1`, pinned by Git tag and Cargo lock to commit
  `25e8a9e431c815363a928349ed8083c6fa357e05`;
- Tokio (resolved to `1.53.1`) for Nemo's async engine interface; and
- Fenix pinned in `flake.lock` at
  `68b310d17eadf75cefc0aa3c16783f3dfb1791f9`.

Nemo requires unstable Rust features. The branch-local Nix shell therefore uses
Fenix's pinned 2026-09-20 nightly: `rustc 1.100.0-nightly
(feaadeeac 2026-09-19)`. No host/global toolchain or sys pin changes are needed.
The locked Cargo package count grows from 53 to 345 (292 transitive additions).
For scale context only—not a performance benchmark—the debug binary observed
during validation was 211 MiB and the external Cargo target directory was 2.1
GiB after checks. Release size was not measured.

## 3. Validation

The durable interface tests cover native Boolean/integer/string facts, recursive
parent traversal, stratified negation, arbitrary titles in native fact
injection, compact/JSON output, base-relation protection, base arity, missing
result declarations, recursive-negation rejection, and rejection of import
directives without running them.
The shared runner additionally covers all three declarations, current-state
mutations, mixed dependency satisfaction, a parent cycle, duplicate paths,
query renaming and constant changes, empty matches/stores, malformed syntax,
unknown names, ordering, and read-only behavior.

Initial candidate checks (external target
`/tmp/graphiga-query-nemo-target`, `CARGO_BUILD_JOBS=2`):

```text
cargo fmt --check                                      PASS
cargo test --locked                                    PASS (1 unit, 9 CLI)
cargo clippy --locked --all-targets -- -D warnings     PASS
cargo build --locked                                   PASS
python3 query-spikes/verify.py --binary \
  /tmp/graphiga-query-nemo-target/debug/graphiga \
  --language datalog                                   PASS
```

## 4. Simplicity review

The required independent simplicity review has not yet run. Source writes pause
at the tested commit before that review. Findings, dispositions, and any tested
but unreviewed repairs will be recorded here after orchestration.

## 5. Awkwardness, removable pieces, and limits

- Nemo's library surface is async because it can support external imports, even
  though this adapter forbids imports. That requires Tokio and makes Graphiga's
  otherwise synchronous entry point async.
- Nemo's dependency closure and nightly requirement dominate integration cost.
  The Fenix `complete` toolchain is reproducible but includes more components
  than Graphiga itself needs; narrowing it is a possible simplicity-review item
  if it remains reliable.
- An empty derived relation appears as no physical predicate table. The adapter
  deliberately maps that engine result to an empty Graphiga result after the
  parsed program has proved that `result/1` is defined.
- Facts are rebuilt in memory for each query. There is no index or cache to
  reconcile with documents, which keeps one authoritative store fact but leaves
  larger-store performance unmeasured.
- Nemo accepts a much broader language than this workload needs. The adapter
  enforces only Graphiga's input/output and no-import/export contract; it does
  not claim hostile-program resource isolation.
- No release build size, throughput, memory benchmark, non-x86_64 Nix shell,
  concurrent query execution, or large recursive workload was tested.
