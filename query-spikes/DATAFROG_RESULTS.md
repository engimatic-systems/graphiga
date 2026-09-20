# Datafrog query spike result

This branch implements the `spike/query-datafrog` comparison arm. It keeps the
Graphiga v0 document store authoritative and evaluates the runtime rule string
selected from `schema.toml` by:

```sh
graphiga --store STORE query QUERY_NAME
graphiga --store STORE query QUERY_NAME --json
```

The committed `query-spikes/datafrog.toml` declarations can be copied into the
store's root `[queries]` table. A query exports unary `result(NodeId)`; output is
the existing ID/type/title record view in ascending ID order.

## 1. Authoring experience and expressive limits

The language supports fixed-arity lowercase relation atoms, uppercase variables,
double-quoted strings, Boolean and signed 64-bit integer constants, `:-` rules,
comma conjunction, multiple rules for union, `not`, and `=`/`!=`. Positive
recursion and stratified negation are supported. Relations are sets.

Graphiga projects these typed input relations for each invocation:

```text
node(Id, Type, Title)
attr(Id, AttributeName, ScalarOrStringSetMember)
edge(SourceId, AttributeName, TargetId)
```

Reference values appear only in `edge`; String-set members and non-reference
scalars appear in `attr`. String, Boolean, and integer values remain distinct.
Undirected reference declarations already stored on both documents therefore
project both directions.

The parser accepts ASCII identifiers and printable ASCII string content, with
`\"`, `\\`, `\n`, `\r`, and `\t` escapes. It intentionally has no comments,
zero-arity atoms, standalone fact syntax, ordering comparisons, functions,
arithmetic, aggregates, parameters, imports, effects, or generated code. A
ground rule can use a constant filter, for example `always("id") :- true =
true.`. Invalid syntax, unsafe variables, inconsistent arities, undefined
relations, base-relation redefinition, a non-unary/missing `result`, and
recursive negation are rejected before document facts affect the answer.

The three workload programs were direct to author. The readiness query needs two
negation strata (`closed` before `unresolved`, then `unresolved` before
`result`), while descendants uses the conventional two-rule transitive closure.
The explicit helpers make evaluation order understandable but are more ceremony
than the predicate arm's graph operators.

## 2. Implementation size, dependency, and integration

Relative to common base `6c59a754c175f0dd6b2c818b6c61114036999620`:

- `src/query.rs`: 1,028 physical lines. This is the full owned frontend and
  evaluator adapter: lexer/parser, typed terms, safety/arity binding,
  stratification, dynamic rule planning, fact projection, Datafrog operation
  plans, result extraction, and diagnostics. Nothing in this count is supplied
  by Datafrog.
- Existing production integration: 35 added lines and one removed line across
  `src/schema.rs`, `src/graph.rs`, and `src/main.rs`.
- `tests/datafrog_query.rs`: 146 physical test lines, in addition to the shared
  black-box runner.
- New direct dependency: `datafrog = 2.0.1` (one crate, no transitive
  dependencies). `Cargo.lock` pins registry checksum
  `a0afaad2b26fa326569eb264b1363e8ae3357618c43982b3f285f0774ce76b69`.

The planner creates a Datafrog variable for each logical relation and explicit
intermediate variables for bindings and join keys. Datafrog's monotone
`Iteration`, `from_join`, `from_antijoin`, `from_map`, and `from_leapjoin`
operators perform fixpoints, joins, antijoins, projection, and filtering.
Negated relations are completed in lower strata and passed as fixed Datafrog
relations. There is no alternate handwritten tuple join/fixpoint path and no
engine trait or runtime engine selector.

Integration points are deliberately local: `Schema` owns query source lookup,
`Graph::query` turns result IDs into the existing record view, and the CLI adds
one `query` subcommand.

## 3. Checks and additional tests

The pinned Nix shell currently supplies rustc/cargo 1.95.0, rustfmt 1.9.0, and
Clippy 0.1.95. Required final checks use
`CARGO_TARGET_DIR=/tmp/graphiga-query-datafrog-target` and
`CARGO_BUILD_JOBS=2`.

The durable public-interface tests cover:

- strings, Booleans, integers, equality/inequality, union, ground rules,
  positive recursion, and stratified negation;
- malformed syntax and unsafe head, negation, and filter variables on an empty
  store;
- inconsistent arities, base-relation redefinition, recursive negation, and a
  valid negative dependency on a base relation.

The shared runner additionally covers all exact fixture results, current-state
mutations, cyclic parent traversal, declaration renaming and constant changes,
unknown and malformed queries on empty input, empty results, ordering, both
output modes, and read-only behavior.

## 4. Simplicity review

Pending the required independent simplicity review. The implementation commit is
the stable review surface; source writes are paused while that review runs.

## 5. Awkwardness and consequential limitations

Datafrog provides efficient typed relational operators but deliberately does not
provide a runtime Datalog language. Dynamic rule text therefore requires owning
all 1,028 lines of frontend, validation, stratification, and runtime planning.
The largest awkwardness is adapting runtime-width rows and bindings to
Datafrog's statically typed key/value operators; explicit keyed intermediate
relations make Datafrog perform the work but add visible planner machinery.

Evaluation rebuilds in-memory facts and plans for each query invocation. That is
appropriate for this read-only spike and avoids another authoritative index, but
it is not a performance claim. Query programs are trusted schema content: there
are no time, memory, rule-count, or recursion resource limits. Each selected
query is parsed and validated when invoked rather than compiling every query at
store load. Unsupported language features are rejected rather than silently
translated or delegated to another engine.
