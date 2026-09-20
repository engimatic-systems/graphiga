# Predicate query spike result

This branch implements the predicate arm of the shared comparison. It adds no
runtime engine selection and no persistent index: a selected schema declaration
is parsed, bound to the schema, and evaluated against the documents loaded for
that invocation.

## 1. Authoring experience and limits

The three declarations remain short and close to the questions they express:

```text
status = open and tags contain backups
status in [open, in_progress] and not any deps (status != closed)
id != demo-epic and reaches parent demo-epic
```

The local operators are direct for scalar and set filtering. `any` makes the
readiness condition readable without introducing a rule relation. `reaches` is
a deliberately fixed transitive operation rather than user-defined recursion.
Both accept only schema-declared `Reference` or `Set(Reference)` attributes and
use undirected attributes as reciprocal relationships.

Declarations have bare tokens, whitespace, parentheses, brackets, commas, and
comparison punctuation as lexical delimiters. There are no quoted literals,
escapes, query parameters, projections, aggregates, arithmetic, incoming walks,
or user-defined functions. Values containing delimiters cannot be expressed.
`not` binds tighter than `and`, which binds tighter than `or`.

Fields and literals are schema-bound before evaluation. Structural `id`, `type`,
and `title` are scalar read fields. Integer comparisons are numeric; other
scalar comparisons use their typed Boolean or lexical order. An absent scalar
does not satisfy a comparison or `in`; use `no FIELD` to select absence. `has`
requires presence and non-emptiness, while present Boolean `false` and Integer
`0` count as present. `any` over no references is false. `reaches` requires one
or more traversals and terminates on cycles.

## 2. Size, dependencies, and integration

Relative to common base `6c59a754c175f0dd6b2c818b6c61114036999620`:

- Production: 789 added lines and 1 removed line (`src/query.rs` plus small
  schema, graph, and CLI integration points).
- Tests: 215 added lines in `tests/query.rs`.
- Dependencies: none added. The implementation reuses the existing `anyhow`,
  `clap`, `serde`, `serde_json`, and `toml` stack.

The owned implementation consists of a tokenizer and recursive-descent parser,
an unbound syntax tree, schema binding to typed fields/literals, direct value
projection from `Document`, nested predicate evaluation for `any`, and a
visited-set traversal for `reaches`. Boolean `not` is ordinary expression
negation; there is no join planner, general fixpoint evaluator, or stratification
machinery. Query results reuse `Graph`'s existing compact record projection and
ID ordering.

The split between syntax and bound expressions accounts for a material part of
the code, but ensures that invalid fields, literals, and operator/field pairings
fail before scanning an empty store. Graph traversal scans loaded documents for
reverse undirected declarations rather than introducing another authoritative
edge index.

## 3. Checks and evidence

The branch uses the pinned Nix development shell, external
`CARGO_TARGET_DIR=/tmp/graphiga-query-predicates-target`, and
`CARGO_BUILD_JOBS=2`.

Final implementation checks:

```text
cargo fmt --check                                      PASS
cargo test --locked                                    PASS
cargo clippy --locked --all-targets -- -D warnings     PASS
cargo build --locked                                   PASS
python3 query-spikes/verify.py \
  --binary "$CARGO_TARGET_DIR/debug/graphiga" \
  --language predicates                               PASS
```

The shared runner covers baseline outputs, mutation-sensitive readiness,
transitive cycles and deduplication, renamed declarations/constants, empty
results/store, diagnostics, ordering, JSON shape, and read-only behavior.
`tests/query.rs` durably covers precedence and parentheses, every local operator,
Boolean/Integer presence, structural fields, scalar and set reference forms,
directed and undirected graph operators, cycles, and schema/type/syntax failures
on an empty store. Existing v0 tests remain unchanged and passing.

Toolchain observed in the pinned shell: rustc 1.95.0, Cargo 1.95.0, rustfmt
1.9.0, Clippy 0.1.95, and Python 3.13.14.

## 4. Simplicity review

Independent simplicity review is pending. No post-review repairs have been made.

## 5. Awkwardness, removable parts, and limitations

Extending the original local language is inexpensive for authors but not free
for maintainers: schema binding and the two graph-aware forms require
substantially more code than the original tko-local parser. The raw/bound tree
split and typed field variants are the clearest removable candidates, but
collapsing them would mix syntax parsing with schema validation or weaken the
fact that a parsed predicate is ready to evaluate.

The evaluator reparses the selected declaration and derives graph relationships
on every process invocation. Reverse undirected lookup is linear in the loaded
node set at each traversal step, so this is suitable for the bounded flat-store
spike, not a performance claim for large graphs. Title predicates also invoke
the existing Markdown title parser per examined node.

Only the documented bare-token language and current schema value vocabulary are
tested. No hostile-query resource limits, Unicode/quoted literal syntax,
parameterization, incoming directed traversal, query caching, or generalized
recursive definitions are provided. As with the v0 graph, well-formed documents
and existing reference targets remain input assumptions.
