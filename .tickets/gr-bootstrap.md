+++
created = "2026-11-06T12:56:09Z"
deps = []
links = []
priority = 1
status = "closed"
tags = ["self-hosting"]
type = "task"
+++
# Bootstrap a small TK wrapper and self-host Graphiga work

## Description

Handwritten first ticket. Use the simpler workflow from ../ticket (194b71a),
which is currently used at work, rather than inheriting TKO's extended model.
Provide a repository-local wrapper around the accepted predicate implementation.
Once this ticket passes, create subsequent work records through that wrapper.

## Design

Keep Graphiga responsible for schema validation, graph relationships, queries,
and persistence. The wrapper supplies IDs, UTC timestamps, partial-ID resolution,
store selection, and command translations. Use Graphiga's structured list output
for resolution; do not add a second frontmatter reader or query evaluator.

Use the three TK statuses open/in_progress/closed and its five types
bug/feature/task/epic/chore. Blocked is derived from direct dependencies, not
stored. A parent does not imply blocking; closing does not require readiness.
All types use Description, Design, and Acceptance Criteria sections.

Pin a separate bootstrap executable so unfinished engine edits do not disable
ticket management. Use .tickets by default, with TICKETS_DIR for isolated stores.
This is a TOML-backed workflow adapter, not a YAML importer or complete TK clone.
Views initially expose ID/type/title in ID order. Defer assignees, note appends,
priority-sorted views, plugins, dependency tree/cycle views, and recently-closed
mtime behavior. Pairwise links only. Single writer; current persistence limits
remain in effect and must be recorded as follow-up work.

## Acceptance Criteria

- Create prints only a generated ID and supplies defaults plus a UTC timestamp.
- All five types work; invalid type/status/priority fails without writing.
- Multiline description and acceptance content survives creation and status edits.
- Start, close, and reopen use Graphiga mutations and preserve body bytes.
- A depending on B is blocked until B closes, then ready, including while A is
  in progress. Closed tickets are excluded from ready and blocked.
- Removing a dependency restores readiness; pairwise links are reciprocal.
- Exact and unique partial IDs resolve; ambiguous/missing IDs fail without writes.
- Invocation works outside the repository and supports isolated temporary stores.
- Bootstrap and compatibility limits are documented; an integration check drives
  the real Graphiga binary through the wrapper.
- Close this ticket through the validated wrapper and create the next tickets
  through it, recording the motivation and acceptance criteria for each.
