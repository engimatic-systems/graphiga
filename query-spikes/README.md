# Two query spikes, one workload

This is a prepared comparison contract, not an implemented query feature.
Both alternatives start from the same committed Graphiga v0, use the same
[tko-shaped store](store/schema.toml), and must pass the same [runner](verify.py).
Only query declaration syntax and its implementation differ.

- `spike/query-predicates`: tko-derived predicates plus bounded graph operators.
- `spike/query-datalog`: restricted textual Datalog evaluated using **Datafrog**.
- [HANDOFF.md](HANDOFF.md) supplies launch authority, worker assignments, and
  reporting expectations. Implementation awaits an explicit operator start.

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

Do not change this fixture, its query meanings, overlays, oracle, or runner to
make one implementation pass. Necessary corrections belong on the shared base
and must reach both branches before comparison.

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

## Datalog alternative

[datalog.toml](datalog.toml) contains the actual query-local programs. Each
program exports a unary `result(NodeId)` relation and may define local helpers.
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

The frontend is deliberately bounded:

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

Use **Datafrog for evaluation**, not merely as an unused dependency around a
handwritten fixpoint engine. Its joins/antijoins and monotone iteration must do
real work. It is a Rust evaluation library, not a textual Datalog parser or
ready-made query-language runtime: the small parser, rule planning, and fact
projection are part of this alternative's implementation cost.

Negated inputs must be fixed relations. Complete lower strata before evaluating
rules that negate them; positive recursive relations may grow within a stratum.
Record the selected crate version and any deliberate subset limitations.

References checked during preparation:

- <https://github.com/rust-lang/datafrog>
- <https://docs.rs/datafrog/latest/datafrog/> (served version 2.0.1)

Do not implement special Rust handlers for the three example queries. The rules
in schema must be interpreted as rules; the experiment should expose the real
cost of generic rule evaluation over this bounded language.

## Checks and comparison report

From the branch checkout in its pinned Nix shell, after building:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
python3 query-spikes/verify.py --binary /absolute/path/to/graphiga --language predicates
# On the other branch, use --language datalog; this flag belongs to the test
# runner only, not to the Graphiga command or a runtime engine abstraction.
```

The pre-implementation preparation check is:

```sh
python3 query-spikes/verify.py --check-fixture
```

It validates TOML and expected results using an independent small oracle. It is
not evidence that either query engine exists or passes its acceptance tests.

Use the same report outline in both branches:

1. Query authoring experience and expressive limits for these three queries.
2. Added production/test lines, dependencies, and integration points relative
   to the common base; numbers inform comparison, not a smaller-is-always-better
   score. Include the cost of any rule frontend/planner.
3. Required checks, shared acceptance results, and relevant additional tests.
4. Simplicity review findings, dispositions, and post-review repairs not re-reviewed.
5. What was awkward, what could be removed, and any consequential limitation.

No new benchmark framework is required. Neither approach wins by declaration;
the operator chooses after inspecting both results. Preserve a usable v0 and
avoid a production query-engine abstraction until the comparison earns it.
