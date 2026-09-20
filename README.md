# graphiga

Graphiga v0 is a small schema-directed document graph. A flat store contains a
`schema.toml` and one TOML-frontmatter Markdown file per node; the graph is
derived from those files.

This repository is a local experimental spike governed by [SPECv0.md](SPECv0.md),
not a production release.

## Development environment

On x86_64 Linux, enter the pinned Cargo/rustc/rustfmt/Clippy shell from this
directory:

```sh
nix develop --no-write-lock-file path:.
```

The explicit `path:` form includes the local flake before it is Git-tracked. This
is a development shell, not a global tool installation.

## Reference stores

The committed [reference stores](examples/README.md) contain the deferred FLAC
inquiry and a tko-shaped ticket example. Use copies for mutation experiments.

The [query-spike comparison](query-spikes/README.md) prepares one shared dataset
and acceptance contract for predicate, Nemo, and Datafrog alternatives. Query execution
is not implemented on this baseline; see the [handoff](query-spikes/HANDOFF.md)
for the three branch assignments.

## Usage

Every command selects a store explicitly:

```sh
cargo run --locked -- --store ./specimen list
cargo run --locked -- --store ./specimen show storage-savings --section Investigation --full
cargo run --locked -- --store ./specimen edges storage-savings related --json
cargo run --locked -- --store ./specimen add storage-savings related processing-cost
```

Create a node with fixed two-value `--set` and `--section` groups:

```sh
cargo run --locked -- --store ./specimen create research storage-savings \
  --title "How does FLAC compression level affect storage size?" \
  --set tags audio,flac \
  --section Question "How do levels 0, 5, and 8 compare?"
```

Run the maintained checks with build output outside the checkout if desired:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

## V0 implementation choices

- `clap` parses the CLI, `markdown-rs` parses headings with its default CommonMark
  profile, and `toml` plus Serde parse and render both TOML formats.
- Node IDs match `[A-Za-z0-9][A-Za-z0-9._-]*`; identity remains the complete
  filename minus the final `.md`. Self-references are rejected. Schema attribute
  names `type`, `id`, and `title` are reserved for document structure.
- Integers are signed 64-bit decimal values. Set members are comma-separated;
  an entirely empty set value means the empty set, while any other empty token is
  rejected. Adding to an absent set creates it; removal leaves a present empty
  set, and only `unset` removes it.
- Duplicate `--set` or `--section` names are rejected. Values beginning with `-`
  are accepted. Line breaks in a creation title are folded to spaces so the title
  remains one plain ATX H1; section content retains its line breaks.
- Lists and edge results are sorted by ID. JSON is an array of
  `{ "id", "type", "title" }` objects. Human-readable rows use the same facts.
- Frontmatter edits deterministically reserialize TOML (including sorted set
  arrays) and preserve the Markdown body bytes. Parsed documents require LF
  `+++` delimiter lines; `show --full` returns the original stored bytes.
- Requests are fully calculated before persistence. Multi-file writes are not a
  transaction: an I/O error reports completed earlier writes and warns that the
  failing file may be partial; no rollback or safe continuation is promised.

The implementation deliberately has no discovery, index, locking, integrity
repair, body-wikilink edges, rename/delete operation, or production durability.
The FLAC inquiry described in the specification has not been run.
