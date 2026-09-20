# Three query spikes, one workload

This is a prepared comparison contract, not an implemented query feature.
All three alternatives start from the same committed Graphiga v0, use the same
[tko-shaped store](store/schema.toml), and must pass the same [runner](verify.py).
Only query declaration syntax and its implementation differ.

- `spike/query-predicates`: tko-derived predicates plus bounded graph operators.
- `spike/query-datalog`: schema-resident textual Datalog using **Nemo**.
- `spike/query-datafrog`: our own bounded frontend/planner over **Datafrog**.
- [HANDOFF.md](HANDOFF.md) supplies launch authority, worker assignments, and
  reporting expectations. The operator has authorized the comparison run.

The ordinary [reference stores](../examples/README.md) are preserved separately.
`store/` is a deliberately expanded copy of the tko example, not a different
schema or a live ticket store. Do not place explanatory Markdown inside it.

## Shared public contract

Named queries are defined as strings in the root `[queries]` table of
`schema.toml`. The branch's executable interprets those strings in its own
language; there is no runtime engine selector or interchangeable-engine layer.

```sh
graphiga --store STORE query ready
graphiga --store STORE query ready --json
```

Results are distinct node IDs rendered in the existing compact ID/type/title
view. JSON is an array of `{ "id", "type", "title" }` records in ascending ID
order, identical in shape to `list --json`. Unknown query names, invalid query
syntax, and invalid field/operator use fail nonzero with a useful stderr
message, including on an empty store. Queries do not change documents or schema.
Stores without `[queries]` continue to support all existing v0 commands.

Queries are data, not hardcoded handlers for `ready` or particular ticket IDs.
Changing a declaration or its name must change behavior without recompilation.
Query evaluation uses current document state; no separately authoritative index
or materialized facts are introduced.

Application transition restrictions remain outside Graphiga. In particular,
query support does not make `status = closed` enforce any section-content gate.

## The exact shared query meanings

| Name | Meaning |
| --- | --- |
| `open_backups` | Nodes whose status equals open and whose tags contain backups |
| `ready` | Status is open or in_progress, and every direct dependency has status closed; an empty dependency set satisfies this condition |
| `epic_descendants` | Nodes that reach demo-epic by following one or more parent references; exclude demo-epic itself |

These are the same operations previously proposed for comparison: local
filtering, dependency-based readiness, and transitive descendants. The statuses,
attribute names, tag, and root are query constants, not engine semantics.
No query parameters, arbitrary projections, aggregates, arithmetic, effects, or
section predicates are required for this increment.

### Dataset

The five original tko examples are extended with four synthetic nodes, all
created through the v0 CLI:

- `demo-phase`: in-progress sub-epic under demo-epic. The task and bug are moved
  beneath it, establishing a real two-hop descendant case.
- `demo-free`: open task under that phase, no dependencies, tagged backups.
- `demo-held`: explicitly blocked task under the root, no dependencies. This
  distinguishes authored status from the presence of unresolved dependencies.
- `demo-other`: open task without a parent, tagged other rather than backups.

The bug depends on both the open task and the closed decision, distinguishing
"all dependencies closed" from "any dependency closed." Research and decision
are closed; the task is open and depends on the closed decision. The root and
phase are in progress without dependencies. This is fictional example data,
not a report of actual backup work.

The independent expected ID sets are in [expected.json](expected.json):

```text
open_backups:     demo-bug, demo-free, demo-task
ready:           demo-epic, demo-free, demo-other, demo-phase, demo-task
epic_descendants: demo-bug, demo-decision, demo-free, demo-held,
                 demo-phase, demo-research, demo-task
```

`verify.py` runs on disposable copies. In addition to baseline results it checks:

- Reopening the decision removes the task from ready and adds the decision.
- Closing the task makes the bug ready (both its dependencies are now closed).
- Introducing a parent cycle still terminates and excludes the root from its
  own descendant result; duplicate paths do not duplicate nodes.
- Renaming a query and changing a query constant work without hardcoded names.
- Invalid syntax and unknown queries fail even on empty input.
- Empty matching results produce `[]`; queries preserve the store bytes.

Do not change this fixture, its query meanings, oracle, or runner to make one
implementation pass. The Datalog implementer may adapt datalog.toml to Nemo's
native syntax without changing names or meanings; no custom compatibility parser
is wanted. That dialect-specific template is owned by the Datalog branch; its
copy on the predicate branch is only a preparatory example. Other necessary
corrections belong on the shared base and must reach all three branches.

## Predicate alternative

[predicates.toml](predicates.toml) contains the actual declarations. Start with
tko's narrow language rather than introducing a new general expression runtime:

```text
status = open and tags contain backups
status in [open, in_progress] and not any deps (status != closed)
id != demo-epic and reaches parent demo-epic
```

Preserve the tko local vocabulary: scalar comparisons `= != < <= > >=`, scalar
`in [values]`, set `contain value`, `has`/`no`, `not`, `and`, `or`, and parentheses.
Precedence is not, then and, then or. Bind field names and value types from the
schema, with structural id/type/title available as read fields. Do not special
case tko priority or its known property names. `has` means present/nonempty, not
truthy: a present Boolean false or Integer zero still exists. Validate the query
independently of whether any nodes happen to be scanned.

Add only the two graph forms needed here:

- `any ATTRIBUTE (predicate)`: at least one referenced target matches the nested
  predicate evaluated with that target as the current node. Empty is false.
  Reference and Set(Reference) fields are permitted; no implicit incoming walk.
- `reaches ATTRIBUTE ID`: the current node reaches that ID via one or more
  traversals of that reference attribute. Cycles terminate through visited-node
  tracking. The outer `id != ...` deliberately excludes the root.

Both read schema-declared reference relationships, including reciprocal edges
when the attribute is undirected. No user-defined recursive predicates or
functions are needed. Document lexical limitations rather than growing a new
escaping language just for this spike. The accepted bare-token declarations
above must work; malformed and type-incompatible queries must fail clearly.

Reference: tko revision `373f5b16f4ebdcd610a57dec224b53230386d51b`,
`src/query.rs` and `SPEC.md` query section. Tko's `ready` calculation lives in
`src/read.rs`, not its predicate language. Local research source is
`/home/agent/sys/repos/tko`; do not modify it or carry ticket-specific evaluation
into Graphiga. Respect source licensing/attribution if reusing implementation.

## Nemo alternative

[datalog.toml](datalog.toml) initially illustrates the query-local programs in
conventional notation. The Datalog implementer must translate it to **Nemo's
native syntax**, using Nemo's existing parser and evaluator. Each program exports
a unary `result(NodeId)` relation and may define local helpers.
The graph projects fixed input facts on demand:

```text
node(Id, Type, Title)
attr(Id, AttributeName, Value)
edge(SourceId, AttributeName, TargetId)
```

`attr` includes non-reference scalar attributes and one fact per String-set
member. Absent attributes contribute no facts. `edge` includes Reference and
Set(Reference) declarations; undirected associations expose both directions.
These are derived views of the same schema/documents, not a new store. Preserve
scalar value distinctions rather than serializing every value to a string.

The workload deliberately uses a bounded part of Nemo's existing language:

- Variables, quoted string constants, scalar Boolean/integer constants, and
  fixed-arity relation atoms; conjunction and multiple rules for union.
- Function-free, safe rules: head, negation, and comparison variables must be
  bound by positive body atoms. No value-generating functions or arithmetic.
- Equality/inequality tests as needed for bounded filtering; no ordering or
  aggregate subsystem is required by this workload.
- Positive recursion and stratified negation. Reject recursive negation instead
  of inventing semantics. Base relations are read-only and cannot be redefined.
- A finite domain from current facts and rule constants. No callbacks, effects,
  scheduler, transition language, generated Rust, or external Datalog service.

Use **Nemo's runtime parser and evaluation engine**. Do not build a bespoke
Datalog parser, planner, or evaluator, nor translate rules into hardcoded Rust
handlers. Adapt native syntax and fact ingestion rather than preserving the old
Datafrog-oriented notation. Keep schema/documents authoritative and facts
in-memory or disposable; no new persistent store or external reasoning service.

Prefer Rust embedding. Pin the selected Nemo release/revision and any required
local toolchain in the branch, documenting integration cost and limitations.
A branch-local reproducible nightly toolchain is allowed if needed; no global
toolchain changes. Establish that parsing, negation, recursion and result
extraction work early. If embedding requires a substantial fork, a custom
frontend, or replacing Nemo, stop and report the concrete blocker rather than
starting another backend survey. Do not vendor or redesign Nemo.

Nemo's existing parser/planner owns rule syntax, safety and stratification.
Small adapter-level checks for the public fact/output contract are appropriate;
reimplementing the language to police every unused feature is not. This is a
trusted-schema spike, not a sandbox for hostile programs. Do not introduce
imports, exports, network access, or other effects in Graphiga's query execution.
Record the supported subset and any limitations honestly.

Reference: <https://github.com/knowsys/nemo>.

Do not implement special handlers for the three example queries. The rules in
schema must execute as rules; measure the adapter and dependency cost rather
than designing an engine abstraction for future replacement.

## Datafrog alternative

The operator added this third arm to evaluate the cost of owning a small Datalog
frontend/planner, independently of adopting Nemo. Use [datafrog.toml](datafrog.toml)
as the schema declarations and the same fact projection, safe function-free
rules, positive recursion, stratified negation and unary result relation described
above. The no-bespoke-frontend requirement applies to Nemo, not this arm.

Implement only a bounded runtime language: uppercase variable names, quoted
string and scalar Boolean/integer constants, fixed-arity atoms, `:-` rules,
comma conjunction, multiple rules for union, `not` negation, and `=`/`!=` filters.
All head, negation and filter variables must be bound by positive body atoms.
Reject unsafe rules, inconsistent arities, base-relation redefinitions and
recursive negation. No functions, value generation, aggregates, callbacks or
external effects. Finite facts/rule constants bound evaluation; relations are
sets. Validate independently of whether the store is empty. Additional syntax
needed solely to expose the three queries is an ordinary implementation choice;
report limitations rather than grow a general language.

**Datafrog must actually evaluate the rules** via its joins/antijoins and monotone
iteration, not be an unused dependency around a handwritten fixpoint engine.
Negated inputs must be completed fixed relations: compute lower strata first.
Do not hardcode the three query names or their bodies in Rust. The schema must
contain executable rule text, not labels for precompiled handlers.

Pin the crate version. Count and explain all owned parsing, binding, planning,
stratification and fact-projection code in the comparison report. Keep this
implementation independent: no engine trait/plugin framework shared with Nemo
or predicates. Reference: <https://github.com/rust-lang/datafrog>.

## Checks and comparison report

From the branch checkout in its pinned Nix shell, after building:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
python3 query-spikes/verify.py --binary /absolute/path/to/graphiga --language predicates
# Nemo: --language datalog; Datafrog: --language datafrog. This flag belongs
# to the test runner, not the CLI or a runtime engine abstraction.
```

The pre-implementation preparation check is:

```sh
python3 query-spikes/verify.py --check-fixture
```

It validates TOML and expected results using an independent small oracle. It is
not evidence that any query engine exists or passes its acceptance tests.

Use the same report outline in all three branches:

1. Query authoring experience and expressive limits for these three queries.
2. Added production/test lines, dependencies, and integration points relative
   to the common base; numbers inform comparison, not a smaller-is-always-better
   score. Include the cost of any rule frontend/planner.
3. Required checks, shared acceptance results, and relevant additional tests.
4. Simplicity review findings, dispositions, and post-review repairs not re-reviewed.
5. What was awkward, what could be removed, and any consequential limitation.

No new benchmark framework is required. No approach wins by declaration;
the operator chooses after inspecting all three results. Preserve a usable v0 and
avoid a production query-engine abstraction until the comparison earns it.
