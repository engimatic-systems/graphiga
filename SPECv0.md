# Graphiga v0: implementation specimen

Status: finalized for handoff to a disposable implementation spike. The operator
has settled the consequential behavior and delegated remaining details to the
implementer. This is not a production contract or permission to merge, migrate
a live store, or execute the FLAC inquiry. Finalizing these documents does not
itself launch implementation. [FORMALISM.md](FORMALISM.md) owns the model and
glossary.

## Purpose and scope

Build a small Rust document-graph tool and learn from using it. The result is a
bounded experiment, not necessarily something to merge into main. It should
answer whether typed documents and schema-defined connections make a useful
tool without domain-specific code or excessive machinery.

V0 covers:

- Filename-derived identity; one TOML-frontmatter Markdown file per node.
- A required H1 title, optional sections, and type-specific section scaffolds.
- Common typed attributes, with requiredness and creation defaults.
- Reference attributes with explicitly directed or undirected semantics.
- Sets, not ordered lists, for collection values.
- Creation, schema-directed attribute updates, document outlines, section reads,
  compact node lists, and connection inspection.

No predicates, workflow engine, ticket compatibility, full-store integrity
checker, corruption repair, daemon, synchronization, or production durability.
Body wikilinks do not yet define graph edges. Existing title/content editing
belongs to ordinary editors; creation may supply initial section content.

## Worked inquiry: FLAC compression levels

Use a real, reproducible question: choosing a FLAC compression level for a fixed
audio collection, comparing levels **0, 5, and 8**.

Two research nodes:

- **Storage savings:** compare output sizes and percentage savings; record input
  hashes, tool versions, and exact commands.
- **Processing cost:** compare repeated encoding and decoding timings on one
  machine; verify losslessness using decoded PCM samples, not WAV container
  bytes.

A decision node references both and eventually chooses a level using the
measurements and an explicit priority. No winner is predetermined. Both research
nodes carry reciprocal `related` references. Conclusions must be scoped to the
tested collection and machine.

**Do not run the inquiry as part of implementing Graphiga.** Collection selection
and benchmarking are deferred. A small fixed redistributable speech/music mix
with provenance is a candidate, not a selected input. The documents below
contain questions and planned methods, not fabricated observations. Findings
and Outcome are intentionally empty.

## Schema and representation

Use an explicitly selected flat store directory with `schema.toml` and node
files. No ancestor discovery or separately authoritative index. Use the schema
shape below for the specimen; incidental parsing choices are delegated.

```text
specimen/
  schema.toml
  storage-savings.md
  processing-cost.md
  compression-choice.md
```

The entire filename minus its final `.md` suffix is the node ID. It is not
repeated in frontmatter. Node type is the structural frontmatter field `type`.
Title is exactly one H1 in the body, never a frontmatter attribute. Changing a
title does not change identity. Frontmatter is TOML enclosed by standalone `+++`
lines at the start of the document; the remaining body is Markdown.

Specimen schema:

```toml
version = 0

[attributes.references]
type = "set"
items = "reference"
direction = "directed"

[attributes.related]
type = "set"
items = "reference"
direction = "undirected"

[attributes.tags]
type = "set"
items = "string"

[attributes.status]
type = "enum"
values = ["open", "in_progress", "done"]
required = true
default = "open"

[attributes.reviewed]
type = "boolean"
required = true
default = false

[types.research]
sections = ["Question", "Investigation", "Findings"]

[types.decision]
sections = ["Question", "Outcome"]
```

Status and reviewed are required, defaulting to `open` and `false` in this
specimen. References, related, and tags are optional.

The core vocabulary is String, Boolean, Integer, Enum, Reference, and Set.
Only Set(String) and Set(Reference) are supported in v0. String attribute values
contain only characters from the inclusive ASCII range `0x2D` through `0x7E`
(`-` through `~`); whitespace and commas are excluded. Titles and section content
are not String attributes and are not subject to this restriction.

The specimen deliberately uses only the five attributes above; do not hardcode
those names or add demonstration attributes merely for type coverage. Every
Reference or Set(Reference) declaration requires explicit directionality.
Singular undirected references implement exclusive pairing as specified below,
even though the FLAC specimen does not exercise them.

Creation obtains every required attribute from an explicit value or its schema
default. A required attribute without either is a request error. Explicit values
override defaults. Optional attributes are omitted unless explicitly supplied;
no implicit empty sets are written. Defaults apply at creation, not as hidden
read-time values for existing files.

TOML arrays serialize sets; ordering is not semantic. Empty and absent remain
distinct attribute states. Both contribute no reference edges. References are
plain node IDs, not wikilink strings.

All declared H2 sections are scaffolded at creation but are optional in existing
documents. An H1-only body is valid when the required frontmatter is present.
Sections and subordinate headings are content, not separate graph nodes.

### `storage-savings.md`

```markdown
+++
type = "research"
status = "open"
reviewed = false
tags = ["audio", "flac"]
related = ["processing-cost"]
+++

# How does FLAC compression level affect storage size?

## Question

For a fixed audio collection, how do levels 0, 5, and 8 compare in output size?

## Investigation

Planned: choose and document a fixed redistributable collection, prepare the
PCM inputs, and record their hashes, tool versions, and exact encoding commands.
Measure output bytes for each level. Report savings relative to the prepared
PCM payload and the reduction relative to level 0. Scope conclusions to this
collection.

## Findings
```

### `processing-cost.md`

```markdown
+++
type = "research"
status = "open"
reviewed = false
tags = ["audio", "flac"]
related = ["storage-savings"]
+++

# What processing cost do FLAC levels 0, 5, and 8 impose?

## Question

On one machine, how do encoding and decoding costs differ across these levels?

## Investigation

Planned: use the same prepared inputs as the storage investigation. Record the
machine, tool versions, commands, and timing procedure; repeat encoding and
decoding measurements. Compare decoded PCM samples with the input samples to
verify losslessness, rather than comparing WAV container bytes. Scope findings
to this collection and machine.

## Findings
```

### `compression-choice.md`

```markdown
+++
type = "decision"
status = "open"
reviewed = false
references = ["storage-savings", "processing-cost"]
+++

# Which FLAC compression level should we use for this collection?

## Question

Given the storage and processing measurements, which of levels 0, 5, and 8
should we choose? State the priority governing the trade-off before selecting
a level; do not choose a winner in advance.

## Outcome
```

Expected graph:

```text
compression-choice --references--> storage-savings
compression-choice --references--> processing-cost
storage-savings --related-- processing-cost
```

There are two directed edges and one undirected edge. The latter has two stored
declarations. The incoming `references` neighborhood of either research node
contains `compression-choice`; no reverse attribute is stored for that purpose.

## Command surface

Examples omit the common `graphiga --store specimen` prefix. Node selection
uses exact IDs, not substring matching.
No ID generator, rename, delete, or node-type-change command is included.

| Command | Meaning |
| --- | --- |
| `create TYPE ID --title TEXT ...` | Create a node, initial attributes, and section scaffold |
| `list [--json]` | Enumerate nodes with compact human-readable or machine-readable output |
| `show ID [--full] [--section NAME]` | Outline or full text, optionally restricted to a section |
| `edges ID ATTRIBUTE [--incoming] [--json]` | Inspect outgoing directed connections or undirected neighbors; optionally directed incoming connections |
| `set ID ATTRIBUTE [VALUE]` | Assign a scalar or comma-separated set; omitted value assigns an empty set |
| `unset ID ATTRIBUTE` | Drop an optional attribute |
| `add ID ATTRIBUTE MEMBERS` | Add comma-separated set members, idempotently |
| `remove ID ATTRIBUTE MEMBERS` | Remove comma-separated set members, idempotently |

### Schema-directed arguments

There is no JSON input mode. Each scalar or complete set is one value argument;
sets use commas within that argument. The schema determines interpretation.
Do not use variable-length shell argument groups or a trailing `--` convention
to delimit attribute assignments.

| Value type | CLI interpretation |
| --- | --- |
| String | One argument satisfying the `0x2D`–`0x7E` character restriction |
| Boolean | One argument, `true` or `false` |
| Integer | One integer argument; implementer chooses and documents range/grammar |
| Enum | One argument matching a declared value |
| Reference | One argument identifying a node |
| Set | One comma-separated argument; order irrelevant and duplicates collapse |

No bracket syntax, CSV quoting/escaping, or inference from a string's appearance.
`false` supplied to a string attribute remains a string. Shell quoting may still
be needed for metacharacters; it does not change the permitted attribute values.
Title and section arguments can contain ordinary prose, including spaces and
newlines. JSON output is independent of input parsing.

```sh
set storage-savings status in_progress
set storage-savings reviewed true
set storage-savings tags audio,flac,compression
add storage-savings tags benchmark,audio
remove storage-savings tags benchmark,audio
set storage-savings tags
unset storage-savings tags
```

The penultimate command assigns the empty set; the last removes the attribute.
Required attributes cannot be unset. `add` and `remove` accept multiple members
in their comma-separated argument and apply only to sets. Duplicate or repeated
inputs do not multiply values or edges. Empty-token and absent-attribute edge
cases are delegated below; they must not introduce ordered-list semantics.

### Creation

Creation requires a node type, ID, and title. It refuses to overwrite an existing
node. Repeated `--set ATTRIBUTE VALUE` groups may initialize **any** declared
attribute, required or optional, using the same schema-directed parsing as
standalone `set`. A set's members occupy one comma-separated value argument;
`--set tags ""` explicitly initializes an empty set.

Repeated `--section NAME TEXT` groups supply initial content for named sections.
The body is created with the H1 and the type's H2 scaffold, with supplied content
placed in the corresponding sections. Subsequent title and body changes are
made with ordinary editors, not a body-editing command surface.

```sh
create research storage-savings \
  --title "How does FLAC compression level affect storage size?" \
  --set status open \
  --set reviewed false \
  --set tags audio,flac \
  --section Question "For a fixed collection, how do levels 0, 5, and 8 compare?" \
  --section Investigation "Planned: document fixed inputs and encoding commands."

create research processing-cost \
  --title "What processing cost do FLAC levels 0, 5, and 8 impose?" \
  --set status open \
  --set reviewed false \
  --set related storage-savings

create decision compression-choice \
  --title "Which FLAC compression level should we use for this collection?" \
  --set status open \
  --set reviewed false \
  --set references storage-savings,processing-cost
```

These commands illustrate creation semantics; they are not byte-for-byte
reproductions of the handwritten examples. Creating `processing-cost` also adds
the reciprocal reference to `storage-savings.related`. The examples supply
required attributes explicitly to demonstrate overrides; omitting those groups
uses the specimen defaults.

`--set` and `--section` groups have fixed argument counts, not variable-arity
member lists. Repeated-name and option-looking-value behavior is delegated to
the implementer and must be documented. `type` and the H1 title are structural
fields, not attributes set through `--set` or the standalone setter.

### Reference mutation

All attribute mutation paths respect schema directionality. There is no raw
reference setter that bypasses reciprocal maintenance.

```sh
add compression-choice references storage-savings,processing-cost
remove storage-savings related processing-cost
add storage-savings related processing-cost
set storage-savings related
```

Directed operations change only the source declaration. Undirected operations
also update the reciprocal declarations in other endpoint files, preserving
unrelated connections. The empty assignment above removes all of the source's
`related` connections and their reverse declarations. Dropping an optional
reference attribute with `unset` likewise removes its connections.

The same behavior applies during creation. A client does not issue two setters
to establish one undirected association.

### Exclusive pairing

Include singular undirected references in the implementation. They are
`Reference` with `direction: undirected`, not another value type:

```toml
[attributes.partner]
type = "reference"
direction = "undirected"
```

Assignment may replace the source's current pairing, but must not displace the
target's pairing with a different node. Refuse the latter conflict before any
writes. Reassigning to the existing counterpart is a no-op. Reassigning to an
unpaired target removes the source's old reciprocal association and establishes
the new one, leaving unrelated pairings unchanged.

```text
Initially: A unpaired; B <-> C
set A partner B: refused, no writes
set B partner A: A <-> B; C unpaired
```

The result is undirected; the mutation has a designated source. The second
command may change three files. It inherits the partial-I/O-failure contract,
not a transaction guarantee. Required-attribute rules continue to apply.

This relationship is deliberately absent from the FLAC specimen. Implement it,
but exercising it is not a required v0 specimen gate; distinguish any additional
checks actually performed from untested behavior in the implementation report.

### Reading documents

`show ID` returns headings only, including the title and subheadings. It does
not print frontmatter or prose. For example:

```text
# How does FLAC compression level affect storage size?
## Question
## Investigation
## Findings
```

`show ID --full` returns the complete stored document, without a generated
metadata header or duplicated attributes.

`--section NAME` selects a section by exact H2 heading text, not fuzzy matching.
It includes subordinate headings/content up to the next heading of equal or
higher level and composes with `--full`:

| Invocation | Result |
| --- | --- |
| `show ID` | All heading lines |
| `show ID --full` | Entire stored document |
| `show ID --section Investigation` | Selected H2 and subordinate heading lines only |
| `show ID --section Investigation --full` | Selected section text, including its heading and subordinate content |

A missing optional section reports section not found; it is not presented as an
empty existing section. Heading-like text in a code fence is not a heading.
Use plain, unformatted heading text by convention. No formatting-normalization
contract is required. If duplicate H2 names occur, use the first match the
implementation encounters: do not add an ambiguity error or duplicate-section
correctness guarantee. Use markdown-rs for parsing; profile details are
implementation choices within these constraints.

### Lists and connections

Human-readable `list` rows contain ID, node type, and H1 title, with no generated
summary or implicit status filter. For example:

```text
compression-choice  decision  Which FLAC compression level should we use for this collection?
processing-cost     research  What processing cost do FLAC levels 0, 5, and 8 impose?
storage-savings     research  How does FLAC compression level affect storage size?
```

`edges ID ATTRIBUTE` reuses this compact node presentation. For directed
attributes it shows outgoing targets; `--incoming` shows sources. For undirected
attributes it shows neighbors, with no incoming/outgoing distinction.

Both support `--json` for machine-readable output describing the same nodes,
not independently stored metadata. The implementer selects and documents the
exact shape and ordering. A JSON array of `{id, type, title}` records, sorted by
ID in both formats, is the recommended small choice. Presentation order does
not give graph sets an ordering. Exact human-readable whitespace is not fixed.

## Mutation and failure contract

Assume a well-formed input store, one writer, and no simultaneous external edits
or schema changes. Do not build a general validation, repair, locking, or
synchronization subsystem for this experiment.

Attribute input is still checked against the schema: unknown attributes, wrong
value types, enum mismatches, and missing required creation values are request
errors. They must be rejected before writes. Prepare the intended changes before
persisting them so client mistakes do not deliberately produce invalid state.

**Frontmatter-only changes must preserve Markdown body bytes exactly.** That
includes title and prose in every affected endpoint file. This requirement is
bounded content preservation, not a mandate for an elaborate editing framework.
TOML formatting/comment policy is delegated. Reserializing frontmatter without
preserving comments or quoting is acceptable for this spike; do not extend that
permission to changing body bytes.

Report I/O failures. Partial multi-file changes after an I/O failure are
acceptable for v0. No rollback, multi-file atomicity, crash recovery, or
production filesystem durability is promised. Such a failure may leave the
store outside the well-formed-input assumption; the experiment does not prove
safe continuation from that state. Do not claim success after an I/O failure.

## Rust module sketch

These responsibilities and their separation are agreed as a v0 starting point,
not a demand for traits, plugins, or a prescribed internal function inventory.

| Module | Responsibility |
| --- | --- |
| `schema` | Attribute types, node-type definitions, required/default rules, and admissible values |
| `document` | Frontmatter parsing/rendering, H1/section reading, scaffolding, and body-preserving attribute edits |
| `graph` | Interpret references and calculate requested changes, including reciprocal updates |
| `store` | Load files and persist calculated changes |
| `main` | CLI arguments, invoking operations, and output presentation |

`graph` calculates changes without writing files; `store` performs writes.
Neither needs to know what research, a decision, or a completed inquiry means.
Schema rules must not be separately reimplemented by each command. Use ordinary
Rust modules; do not introduce substitutable backends or a second authoritative
graph database for this experiment.

### Dependency choices

Use `clap` for the command line and `markdown-rs` for Markdown parsing. Adopt an
existing Rust TOML parsing library for both schema and frontmatter rather than
writing a TOML parser; the implementer selects the library and records the
choice. Versions and parser
configuration are ordinary implementation choices. Keep schema interpretation
and narrow frontmatter handling local without reimplementing these libraries.

## Acceptance walkthrough

Use disposable stores; no live tickets or audio benchmarking:

1. Load the handwritten FLAC specimen. Observe two directed edges and one
   undirected edge, with the expected incoming and neighboring nodes.
2. Create both node types with H1 titles, required attributes, and scaffolded
   optional sections. Exercise defaults and explicit overrides, optional
   initial attributes, and multiple `--section` groups.
3. Edit or remove optional sections through an ordinary editor. Exercise all
   four `show` combinations, including subordinate headings and fenced examples.
4. Exercise schema-directed comma-separated assignment, empty-set assignment,
   dropping optional attributes, idempotent multi-member addition/removal,
   duplicate member collapse, and the restricted String character range.
5. Change directed and undirected reference sets through creation and attribute
   operations. Verify reciprocal changes and untouched unrelated connections.
   Exclusive pairing is implemented but outside this specimen's exercise gate.
6. Check exact preservation of every affected body's bytes on attribute edits.
   Incorrect requests must not change files; induced I/O failures must be
   reported without claiming atomicity or successful rollback.
7. Verify compact lists and machine-readable output describe the same node IDs,
   types, and titles. No independent title/frontmatter copy is introduced.

Tests belong with the experimental Graphiga implementation and should exercise
its public behavior. Supplementary minimal schemas may cover primitive types
absent from the FLAC specimen; do not inflate the worked example for coverage.

Ask the implementer to return usage examples, tests and results, and a short
account of awkward rules, accidental domain-specific cases, and opportunities
to simplify. The implementation may be revised or discarded rather than merged.

## Implementation discretion and handoff

No further design round is required for incidental details. Choose simple
behavior consistent with this document and record the choices in implementation
usage/help and the result report. In particular:

- Select an ID grammar, integer range, TOML library, and dependency versions;
  use the selected clap/markdown-rs stack. Preserve full
  filename-derived identity and the flat-store restriction; no normalization
  that aliases distinct filenames.
- Decide empty-token and absent-set behavior, self-reference, and remaining
  request edge cases. Recommended defaults: add to an absent set creates it;
  remove from an absent set is a no-op; removal leaves a present empty set;
  only unset drops it. Refuse requested nonexistent targets before writing.
- Choose repeated-group and option-looking-value handling without reintroducing
  variable-arity assignments, JSON input, or CSV escaping. Rejecting duplicate
  group names is sufficient. Attribute declarations remain common to all types.
- Choose the markdown-rs profile, additional-section handling, and precise
  source-slice/newline conventions. Honor exact H2 selection, first encountered
  duplicate match, unformatted-heading convention, and body preservation.
- Choose output shapes, stable presentation order, and diagnostics. JSON arrays
  of compact records and ID ordering are recommended, not another model fact.
  Use stdout for results and stderr for errors; success/no-op exits zero and
  errors nonzero. An I/O failure report should identify known partial progress
  without implying rollback. Exact nonzero code taxonomy is incidental.

The spike should return code, usage examples, checks/results, implementation
choices, and known unexercised paths, including exclusive pairing if not tested.
Do not claim a passing specimen demonstrates features it does not exercise.

Body wikilink graph semantics, production durability, and actually running the
FLAC inquiry remain deferred. No merging, release, live migration, benchmark
execution, or broad architecture expansion is part of this handoff.
