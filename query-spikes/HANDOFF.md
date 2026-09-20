# Query comparison: ready-to-launch handoff

Prepared only. **Do not launch implementation until the operator explicitly
starts orchestration.** The operator authorized committing/pushing this
preparation and selected Datafrog for the Datalog alternative. No winner, merge,
release, or production migration is authorized.

System-of-record tickets in sys:

| Role | Ticket | Branch |
| --- | --- | --- |
| Comparison / operator decision | sys-2dxc | — |
| Preparation | sys-gi61 | spike/graphiga-v0 |
| Predicate implementation | sys-w44b | spike/query-predicates |
| Datafrog implementation | sys-9tqt | spike/query-datalog |

`origin` is `https://github.com/engimatic-systems/graphiga.git`.
The two implementation branches are prepared at the same common commit. Their
Git history, not separate snapshots/manifests, identifies the baseline and
subsequent tested/reviewed candidates.

## Intent

Implement and compare two ways to execute named schema-resident queries against
one fixed tko-shaped dataset. Read [README.md](README.md), the branch's query
declarations, [expected.json](expected.json), and [verify.py](verify.py) completely.
The query meanings, dataset, invocation/output, and shared tests are common.
Only language-specific declarations and implementation differ.

Preserve the already working v0 backend and its two reference stores. No engine
switch, plugin trait, application transition rules, section predicates, query
parameters, aggregate subsystem, or generalized orchestration language is needed.

## One example

Both branches must support:

```sh
graphiga --store STORE query ready --json
```

With the shared fixture and that branch's query declarations installed into
`schema.toml`, the result is the same five ID/type/title records for:

```text
demo-epic, demo-free, demo-other, demo-phase, demo-task
```

Reopening demo-decision changes the result without recompiling: demo-task drops
out and demo-decision enters. Both implementations must derive this from the
schema query and current references/statuses, not recognize `ready` by name.

## Important invariants

### Inputs and scope

- Use the current committed `SPECv0.md` and `FORMALISM.md`, then this comparison
  contract. Original v0 non-query behavior and all its tests must continue to
  work. Additional query modules are fine; speculative engine interfaces are not.
- Read and apply `/home/agent/sys/skills/rust-style/SKILL.md`. Also read the
  relevant formalism/codebase-design guidance and `docs/workflows.md` in sys.
  Do not replace semantic types with untyped bags merely to shorten a report.
- Query source lives in `[queries]` in the schema. The templates in this folder
  are the committed declarations used by the shared runner. Queries are pure
  reads; transition guards stay application-level.
- Use the existing clap, markdown-rs, TOML library, and pinned Nix environment.
  The Datalog branch additionally uses **Datafrog** for real evaluation. Its
  frontend/rule planning cost must be visible in the result; no hardcoded query
  handlers or externally generated Rust standing in for schema interpretation.
- Keep query-spikes/store, expected.json, verify.py, both declaration templates,
  reference stores, and the shared contract unchanged in implementation commits.
  Raise genuine contract/fixture defects to the primary; fix them once on the
  shared baseline and bring the same correction to both alternatives.
- Do not reopen incidental syntax/formatting questions already delegated by the
  contract. Return material scope or model changes instead of guessing authority.

### Working copies and publication

At launch, the primary may create two worktrees for the already prepared branches:

```text
/home/agent/sys/working/sys-2dxc/predicates   spike/query-predicates
/home/agent/sys/working/sys-2dxc/datalog      spike/query-datalog
```

These are implementation worktrees, not snapshot/provenance copies. Each branch
has exactly one writable source checkout. The primary checkout at
`/home/agent/sys/repos/graphiga` remains on the shared baseline branch. Reconcile
existing paths/branches before creating anything; never overwrite user work.

Once explicitly started, implementers may edit their source, Cargo metadata and
lockfile, tests, and branch-specific usage/result documentation. They may commit
and push only their assigned spike branch to origin. No force push, PR, merge,
main update, global installation, source cleanup/reset, sys pin changes, live
store mutation, or FLAC benchmarking. Do not sync or commit the shared ticket
store as part of source publication.

Use separate external Cargo target directories to avoid branch build contention.
Build caches are allowed. Keep examples and test mutations in disposable copies.
Do not add snapshot/manifests or repeated source hashing to this experiment.

### Persistent roles and one review per alternative

Use a persistent implementer/reviewer pair **for each alternative**, with both
roles `openai-codex/gpt-5.6-sol`, high thinking, and direct return messages to the
primary. No silent model substitutions. Independent alternatives may run in
parallel because their writable worktrees are separate; do not share an
implementer's context with its reviewer or make one implementer review itself.

The primary orchestrates, dispositions findings, and checks integration-critical
evidence. It does not implement either engine or add a hidden code-review pass.
Start the assigned ticket before delegation and record worker identities.

Keep the rapid-feedback v0 pattern: **one independent simplicity-focused pass
per alternative**. Pause source writes while that branch is reviewed; a tested
Git commit is enough. Focus on avoidable state, duplicate rules, unjustified
abstractions, and frontend/evaluator complexity. Report material correctness
problems encountered, but do not expand into an exhaustive production audit.

The original implementer may apply accepted bounded repairs and rerun checks.
There is no automatic second review: identify repairs as tested but not
independently re-reviewed. Stop for operator input if another pass is needed.
Neither workers nor the primary select a winning engine or merge it merely
because tests pass. Workers do not close tickets.

## Validation and return expectations

From the branch checkout:

```sh
nix develop --no-write-lock-file path:.
# Use a per-branch external CARGO_TARGET_DIR.
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
python3 query-spikes/verify.py --binary "$CARGO_TARGET_DIR/debug/graphiga" --language predicates
# Datafrog branch: identical command with --language datalog.
```

Create/update Cargo.lock normally when adding dependencies before using locked
checks. Record actual tool/dependency versions. Add durable tests at the query
interface for the language semantics and error handling; the common runner is
necessary but not a substitute for every new grammar/operator obligation.

Report ordinary command results, logs, and Git commit IDs, not source manifests.
Use [README.md](README.md)'s shared comparison outline. Include the parser,
fact projection, joins/fixpoint/negation machinery, not just query text or crate
line count. No invented benchmark claims. Record any known untested behavior.

Each implementer returns a tested commit and concise report, pauses source
writes, and remains available for accepted repairs. Each reviewer returns its
reviewed commit, findings with evidence/severity, and limits. The final primary
report identifies the final tested and reviewed commits, dispositions, and any
unreviewed repairs for both alternatives. Keep the implementation tickets open
for operator acceptance unless separately authorized to close them.

## Branch-specific assignment

### sys-w44b: predicates

Implement the existing tko local grammar with schema-directed field/value
interpretation and the two bounded graph operators specified in README.md.
There is no freeform evaluator or need for recursive user definitions. The
language must interpret its declarations rather than special-case the three
query names, statuses, relation names, or fixture IDs. Document lexical/operator
limits and the cost of extending the original language.

### sys-9tqt: Datafrog

Implement the restricted function-free Datalog frontend and schema-derived fact
projection described in README.md. Use Datafrog joins/antijoins/iteration for
evaluation, including positive recursion and stratified negation. Datafrog is
not a parser; do not imply it supplies your frontend or stratification logic.
Bound the language to the written contract and return meaningful limitations
instead of growing a general programming runtime.

The prepared templates and expected results are shared inputs. No implementation
worker has been launched by preparing or publishing this packet.
