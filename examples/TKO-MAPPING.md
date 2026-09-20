# TKO schema research for Graphiga mapping

## Scope and revision

Read-only inspection of `/home/agent/sys/repos/tko` at verified HEAD
`373f5b16f4ebdcd610a57dec224b53230386d51b`. Sources inspected were
`SPEC.md`, `src/storage.rs`, `src/write.rs`, `src/read.rs`, `src/lint.rs`, and
the minimal CLI/note code needed to resolve current behavior. The comparison is
against Graphiga's `SPECv0.md` and `README.md`; Graphiga implementation files
were not inspected.

## Structural identity and title

Map these as Graphiga structure, not attributes:

- **ID:** Graphiga filename identity. TKO normally writes `TKO_ID`, but its
  loaded ID falls back to the filename stem when that property is absent.
- **Type:** Graphiga structural node type, normalized from `TKO_TYPE`.
- **Title:** Graphiga H1, corresponding to TKO's first top-level Org heading.

This is not exact compatibility. TKO command resolution uses exact or
case-sensitive substring matches against filename stems, while its loaded and
displayed ID comes from `TKO_ID` when present. Graphiga has one exact filename
identity with grammar `[A-Za-z0-9][A-Za-z0-9._-]*` and reserves `type`, `id`,
and `title` for structure. TKO returns `Untitled` when it cannot find a nonempty
`* ` title line; Graphiga requires one H1.

Sources: `repos/tko/SPEC.md:93,144-157,215-223`;
`repos/tko/src/storage.rs:143-165,349-370,560-566`;
`repos/graphiga/SPECv0.md:70-74,233-235`;
`repos/graphiga/README.md:54-56`.

## Node types and sections

Canonical Graphiga type declarations should contain exactly the TKO create
scaffolds, not every allowed heading.

| Type | Create scaffold, in order | Allowed semantic headings | Required nonempty at create | Required nonempty at close |
|---|---|---|---|---|
| `bug` | Description; Scope; Design; Acceptance Criteria | scaffold + Notes | — | — |
| `feature` | Description; Scope; Design; Acceptance Criteria | scaffold + Notes | — | — |
| `task` | Description; Scope; Design; Acceptance Criteria | scaffold + Notes | — | — |
| `chore` | Description; Scope; Design; Acceptance Criteria | scaffold + Notes | — | — |
| `epic` | Description; Scope; Design; Acceptance Criteria; Not yet specified; Decisions | scaffold + Notes | — | — |
| `decision` | Question; Resolution | Description; Question; Options; Resolution; Notes | Question | Resolution |
| `research` | Question; Findings | Description; Question; Findings; Notes | Question | Findings |
| `incident` | Impact; Resolution | Description; Impact; Findings; Resolution; Notes | Impact | Resolution |

TKO's distinctions are explicit in `TypeShape`: scaffold, allowed,
required-at-create, and required-at-close are separate fields. Graphiga v0 only
uses a type's section list as its create scaffold and makes sections optional in
existing documents. Consequently the schema cannot encode allowed-but-not-
scaffolded headings or either nonempty-content requirement. Keep those as
separately documented TKO policy gaps.

Sources: `repos/tko/SPEC.md:168-191,225-250,305-327`;
`repos/tko/src/storage.rs:179-262`;
`repos/graphiga/SPECv0.md:128-140`.

## Attribute mapping

Normalize `TKO_*` keys to lowercase snake_case. In particular,
`TKO_EXTERNAL_REF` maps to `external_ref`, not the query JSON spelling
`external-ref`.

TKO has no universal load-time required-property invariant. The table therefore
distinguishes canonical create output from compatibility read behavior.

| TKO property | Graphiga name and type | Canonical TKO create | Missing-property read behavior |
|---|---|---|---|
| `TKO_ID` | structural ID, not an attribute | generated and written | filename stem |
| `TKO_STATUS` | `status`: Enum(`open`, `in_progress`, `blocked`, `closed`) | always `open` | `open` |
| `TKO_DEPS` | `deps`: Set(Reference), directed | always present as empty list | empty |
| `TKO_LINKS` | `links`: Set(Reference), undirected | always present as empty list | empty |
| `TKO_CREATED` | `created`: String | generated UTC timestamp | absent |
| `TKO_TYPE` | structural type, not an attribute | always written; default `task` | `task` |
| `TKO_PRIORITY` | `priority`: Integer | always written; default `2` | `2` |
| `TKO_ASSIGNEE` | `assignee`: optional String | written only when nonblank | absent |
| `TKO_EXTERNAL_REF` | `external_ref`: optional String | written only when nonblank | absent |
| `TKO_PARENT` | `parent`: optional Reference, directed | resolved existing filename stem when supplied | absent |
| `TKO_TAGS` | `tags`: optional Set(String) | written only when nonempty | empty |

For canonical-new-node behavior, `status` should be required with default
`open`, `priority` required with default `2`, and `created` required but supplied
explicitly because Graphiga has no dynamic current-time default. Canonical TKO
also materializes empty `deps` and `links`; making these required with empty-set
defaults is closest if the schema accepts typed set defaults. Making them
optional better matches TKO's tolerant reader but not its canonical writer.

Sources: `repos/tko/SPEC.md:37-96,193-213,269-303`;
`repos/tko/src/storage.rs:74-94,349-403,533-566`;
`repos/tko/src/write.rs:54-115`;
`repos/tko/src/read.rs:157-207`;
`repos/graphiga/SPECv0.md:95-136`.

### Status and priority restrictions

Status mutations accept only `open`, `in_progress`, `blocked`, and `closed`.
Priority is `0` through `4`, where `0` is highest and `2` is the default. These
restrictions are enforced on normal create/mutation paths, not fully on existing
files: status and type parse as arbitrary strings, while an existing priority
may be any parseable `u8`. Unknown types skip typed lint.

Graphiga Enum represents the status values. Graphiga's signed 64-bit Integer
cannot encode priority's `0..=4` range; an Enum of string digits would enforce
the range but lose numeric representation.

Sources: `repos/tko/SPEC.md:159-213`;
`repos/tko/src/write.rs:634-655`;
`repos/tko/src/storage.rs:551-557`;
`repos/tko/src/lint.rs:169-178`;
`repos/graphiga/README.md:57-60`.

## Relationship semantics

- `deps` is directed from a ticket to the ticket it depends on.
- `parent` is singular and directed from child to parent. TKO resolves the
  supplied parent to an existing filename stem during creation; there is no
  reciprocal children property.
- `links` is symmetric. `link` and `unlink` update both endpoint files.

Normal dependency/link mutations resolve both endpoints, reject self-relations,
preserve insertion order, and do not introduce duplicate entries. TKO stores
bracketed ordered lists and parses them into `Vec<String>`; hand-edited
duplicates remain possible. Graphiga Reference directionality captures the edge
meaning, but Graphiga sets are unordered. Graphiga also distinguishes absent
from present-empty sets, whereas TKO reads an absent list property as empty.

Sources: `repos/tko/SPEC.md:394-435`;
`repos/tko/src/storage.rs:376-403,544-548`;
`repos/tko/src/write.rs:338-408,451-466`;
`repos/graphiga/SPECv0.md:122-136,336-351`.

## Notes convention and heading policy

TKO notes conventionally appear as:

```org
** Notes
*** [2026-06-11 Thu 18:20Z] Title
Free-form body
```

`Notes` is a level-2 semantic section and note entries are direct level-3
headings. Added notes use UTC timestamps. Current write code requires a nonempty,
one-line title of at most 72 Unicode scalar characters. Deeper headings within
a Notes subtree are free-form and excluded from semantic-heading lint; level-2
headings remain in scope.

Graphiga can preserve this as Markdown content but cannot schema-encode the
level-3 entry protocol, timestamps, title limit, note matching, or the lint
exemption. `Notes` should therefore remain an allowed-policy concept rather than
part of canonical create scaffolds.

Sources: `repos/tko/SPEC.md:244-250,638-700,834-886`;
`repos/tko/src/write.rs:502-553`;
`repos/tko/src/lint.rs:139-167,217-250`.

## Create, close, and lint policy not represented by Graphiga schema

TKO creation additionally enforces:

- trimmed nonempty title;
- valid type and priority;
- escaped `\\n` expansion and trimming before section validation;
- rejection of section flags foreign to the selected type;
- nonempty Question for decision/research and Impact for incident;
- existing-target resolution for parent.

Closing additionally enforces nonempty Resolution for decision/incident and
nonempty Findings for research. `--reason` fills, appends to, or creates the
required close section before setting status; it is rejected for types without a
close-required section. `status ID closed` runs the same close gate.

Lint separately enforces semantic-heading uniqueness and level-2 placement,
required-section content, foreign-section warnings, and the note-title limit.
Semantic heading matching is ASCII case-insensitive after trailing-space
trimming. Graphiga has no workflow engine or ticket compatibility layer, so
these remain application/lint policy rather than structural schema.

Sources: `repos/tko/SPEC.md:235-252,305-327,335-392,834-892`;
`repos/tko/src/write.rs:54-70,118-155,177-246`;
`repos/tko/src/lint.rs:111-213`;
`repos/graphiga/SPECv0.md:27-30,138-140`.

## Graphiga String and identity constraints

Graphiga String attributes permit only characters in inclusive ASCII
`0x2D` through `0x7E`; whitespace and commas are excluded. Titles and section
content are not subject to that restriction.

Consequences for the mapping:

- canonical timestamps such as `2026-06-11T18:20:12Z` are representable;
- assignees containing spaces, such as `Ada Lovelace`, are not;
- Unicode assignees, external references, and tags are not;
- TKO tags containing internal spaces or lower ASCII punctuation are not;
- TKO's existing filenames or `TKO_ID` values need not satisfy Graphiga's ID
  grammar, although ordinarily generated lowercase ASCII IDs do.

TKO trims optional scalar values and comma-split tag members but otherwise has
no corresponding Graphiga String character restriction. Graphiga also cannot
express dynamic `now` or Git-user defaults in static schema data.

Sources: `repos/graphiga/SPECv0.md:116-132,255-268`;
`repos/graphiga/README.md:54-63`;
`repos/tko/src/storage.rs:376-394,537-548`;
`repos/tko/src/write.rs:85-100`.

## Pinned-code differences from `SPEC.md`

Two representation-adjacent discrepancies should not be silently copied from
the prose specification:

1. `SPEC.md` says create defaults the title to `Untitled` and assignee from
   `git config user.name`. At the pinned HEAD, the CLI requires a positional
   title and passes only an explicitly supplied assignee. The parser still uses
   `Untitled` when reading a body without a title.
2. `SPEC.md` describes positional/stdin note text and timestamp-only notes. The
   current CLI is `add-note ID --title TITLE [--body BODY]`, and the writer
   rejects an empty title, making timestamp-only notes unavailable through the
   current command.

Sources: `repos/tko/SPEC.md:269-303,638-679`;
`repos/tko/src/cli.rs:74-101,169-176,322-327,387-402`;
`repos/tko/src/storage.rs:560-566`;
`repos/tko/src/write.rs:502-536`.
