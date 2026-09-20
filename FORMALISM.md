# Graphiga: a small document graph

Status: finalized for the v0 spike handoff, not a claim about an implemented
system or a permanent architecture. [SPECv0.md](SPECv0.md) contains the worked
specimen and command contracts. Incidental choices are delegated to the
implementer; the model remains revisable in light of the experiment.
Decision history lives under `ledger list graph --all` in the sys workspace;
`sys-93oc` records the documentation work.

## Purpose and scope

Graphiga represents a graph in plain-text documents. One Markdown file with
TOML frontmatter represents one node. A TOML store schema defines node types,
type-specific sections, and attributes common to all nodes. Frontmatter is
delimited by `+++` lines; the schema lives in `schema.toml`.

Research, decisions, tickets, and workflow statuses are meanings supplied by a
store specification and its users, not concepts built into Graphiga. The v0
experiment has no predicates, calculated workflow state, or orchestration. A
separate tko client is a possible future consumer, not a compatibility target.

The guiding principles are:

- Keep as few independent facts as possible; derive the rest.
- Prefer representations that make correctness true by construction.
- Assume a well-formed store rather than build speculative corruption handling.

## Vocabulary

| Term | Meaning |
| --- | --- |
| Store | Documents governed by one schema. |
| Node | One identified entity in the store. |
| Document | The Markdown file representing a node. |
| Node ID | The filename-derived identity of a node. |
| Node type | A schema-defined category selecting the section scaffold. |
| Title | The required H1, providing the node's human-readable label. |
| Section | A heading and its content, including subordinate headings and content. |
| Attribute | A named, schema-typed frontmatter value. |
| Value type | The permitted shape of an attribute value. |
| Reference | A value identifying a target node by ID. |
| Reference declaration | A source node naming a target under a reference attribute. |
| Edge kind | The attribute name identifying the relationship. |
| Edge | A directed or undirected connection of a particular kind. |
| Exclusive pairing | A singular undirected reference: each endpoint has at most one counterpart. |
| Graph | Nodes and edges interpreted from documents under the schema. |

Node and document are graph and file perspectives on the same entity, not two
records to synchronize. Sections have no independent node identities. Node type
and value type are distinct uses of the word "type."

## Store, identity, and title

Write a store as:

```text
S = (schema, documents)
```

Documents are authoritative. The graph is derived, not another independently
maintained truth. Under the working layout of a flat directory of node files:

```text
ID("storage-savings.md") = "storage-savings"
```

Remove only the final `.md` suffix; do not truncate or normalize the remaining
name. A directory cannot contain two entries with the same exact filename, so
this representation establishes identity uniqueness without an ID registry or
frontmatter/filename agreement check. The implementer selects and documents a
filename grammar consistent with this flat layout; nested-directory discovery
is not promised.

Renaming a node file changes its identity. Changing its title does not. Rename
and reference-rewriting operations are outside the v0 command surface.

A document contains:

```text
document = (node type, attribute values, Markdown body)
```

The structural frontmatter field `type` selects the node type. The body contains
exactly one H1 title. There is no frontmatter `id` or `title` attribute. Compact
list views derive ID, node type, and title from these existing facts.

Other sections are optional. A title-only body can be valid, provided required
frontmatter attributes are present. Creation scaffolds the declared sections;
subsequent title and body changes belong to ordinary editors.

## Schema and values

A schema supplies:

```text
T     node-type names
A     attribute declarations shared by every node type
H(t)  ordered section scaffold for each node type t in T
```

Selecting a node type selects its section scaffold, not a different attribute
schema. The initial specimen has research sections Question, Investigation,
Findings and decision sections Question, Outcome. Each inquiry remains one node.

The value vocabulary is:

```text
String | Boolean | Integer | Enum(allowed values) | Reference | Set(element type)
```

There is no List type in v0. A Set is unordered and has distinct members. TOML
arrays serialize sets without making their order significant. Only
Set(String) and Set(Reference) are supported; no nested sets.

String attribute values use only ASCII characters in the inclusive range
`0x2D` through `0x7E` (`-` through `~`). Commas and whitespace are excluded.
This restriction does not apply to titles or section content. The CLI uses
commas to separate set members in one argument, not JSON or CSV escaping.

Attributes may be required or optional. Required attributes may have creation
defaults. Creation uses explicitly supplied values in preference to defaults
and must obtain a value for every required attribute. Optional attributes are
absent unless explicitly supplied. Defaults are a construction convenience, not
another source of values consulted when reading an existing document.

`Reference` is one value type. Every Reference or Set(Reference) attribute must
explicitly declare `direction: directed` or `direction: undirected`. Direction
belongs to the schema, never to individual stored reference values.

## Reference declarations and edges

For reference attribute `a`, write:

```text
R_a(u, v) = node u declares a reference to node v under attribute a
```

A singular reference names one target. A reference set names distinct targets
without ordering them. Attribute names distinguish edge kinds: connections
under `references` and `related` are not the same edge.

This relation is notation for document facts, not another storage structure.
For a directed attribute:

```text
R_a(u, v)  means  u --a--> v
incoming_a(v) = { u | R_a(u, v) }
```

Incoming edges need no stored reverse attribute. A reverse declaration is a
separate directed edge; reciprocal directed declarations do not become
undirected. No acyclicity rule follows from directionality.

For an undirected attribute, the representation is reciprocal:

```text
R_a(u, v) iff R_a(v, u)
```

Together, these declarations represent one edge of kind `a` between unordered
endpoints `{u, v}`. Both files carry the association. They are intentionally
redundant representations of one fact, not independent choices about its
existence. V0 assumes reciprocity; it does not audit or repair external
inconsistencies.

Body wikilinks remain an open modeling question. They are not implicitly
frontmatter references, dependencies, or related edges in this experiment.

## Attribute operations

For a set-valued attribute with current value X and supplied members M:

```text
set(M):     X' = M
add(M):     X' = X union M
remove(M):  X' = X minus M
```

Repeated members have no additional effect. Addition and removal are idempotent;
reordering members does not change a value or the graph.

`set` on a scalar assigns one value. `set` on a set with no members assigns the
empty set. `unset` removes an optional attribute, rather than assigning an empty
value. Required attributes cannot be unset. Empty and absent are distinct
attribute states even though neither contributes reference edges.

For undirected reference sets, assignment changes reciprocal declarations too:
new targets acquire the reverse reference; removed targets lose it. Unrelated
connections remain unchanged. Clients do not perform two independent setters to
create one undirected edge. Reference attributes have no raw mutation path that
bypasses their directionality.

The same attribute semantics apply to initial `--set` groups during creation.
Creation may also initialize section content with `--section` groups; this does
not introduce commands for editing existing bodies.

## Consequential interactions

### A set is not an ordered collection

Assigning tags `audio,compression,audio` yields the same value as assigning
`compression,audio`. Stable sorting for display is presentation, not additional
stored knowledge. The same equality rule applies to reference sets.

### Undirected changes involve both documents

Removing a related connection between two distinct nodes changes both files.
Its other edge kinds and both document bodies remain unchanged. Successful
frontmatter-only operations must preserve Markdown body bytes exactly.

This operation is not a filesystem transaction merely because the model names
it as one change. Under the accepted v0 failure contract, an I/O failure may
leave only some affected files changed.

### Exclusive pairing has an undirected result and a designated source

Singular undirected references are supported in the spike. If A names B, B must
name A. A third distinct node C cannot also name B under that attribute in a
well-formed store: B cannot name both A and C.

Assignment may replace the source's current pairing, but must not displace the
target's pairing with a different node. Resolve that conflict before writing.
Assigning an existing counterpart is a no-op. For example:

```text
Initially: A unpaired; B <-> C
set A partner B: refused; B already has a different counterpart
set B partner A: A <-> B; C unpaired
```

The second command explicitly replaces B's association; it updates B and A and
removes C's reverse declaration. There is no change to unrelated pairings.
This is `Reference` with `direction: undirected`, not a new value type.

The implementation includes this behavior, but the FLAC specimen does not
exercise it and cannot be cited as evidence that it works. Its multi-file
writes retain the same partial-I/O-failure limits as other reciprocal edits.

## Assumptions and guarantees

The input store is assumed well-formed: documents conform to the schema,
references identify nodes, and undirected declarations are reciprocal. V0 has
one writer, with no simultaneous external edits or schema changes.

Filename identity establishes uniqueness under the stated layout. It does not
establish reference existence or reciprocity; those are assumptions about the
authored store. Schema-directed commands must not deliberately introduce invalid
values, and successful writes must preserve the model's relationships.

Ordinary I/O failures must be reported, but v0 tolerates partial multi-file
changes. Atomicity, rollback, crash recovery, and production filesystem
durability are deferred. An unsuccessful write may leave a store outside the
well-formed-input assumption; continuing safely from that state is not proved
by this model.

Frontmatter-only edits preserve body bytes exactly on successful writes. This
is a bounded content-preservation requirement, not a claim of crash safety or
protection against an external editor racing a write.

## Implementation latitude and deferred work

[SPECv0.md](SPECv0.md) delegates incidental parser, formatting, and presentation
choices to the implementer. Record those choices rather than reopening the
whole design or introducing speculative machinery. Body-reference semantics,
production durability, and actually running the research inquiry are deferred.

Use plain unformatted heading text by convention. Section lookup uses exact H2
text; if duplicates occur, use the first match encountered by the implementation.
There is no duplicate-section correctness guarantee or ambiguity-checking
requirement for v0.

No implementation behavior is evidenced by this document. The experiment will
test whether the chosen interface realizes the model without unnecessary
machinery; finalizing the handoff does not preclude revising the model.
