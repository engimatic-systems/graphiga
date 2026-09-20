# Reference stores

These are ordinary Graphiga stores, created through the v0 CLI after writing
their schemas. Keep README files outside the store directories: each `.md` file
inside a store represents a node.

From the repository root, in the development shell:

```sh
cargo run --locked -- --store examples/research list
cargo run --locked -- --store examples/research show storage-savings --full
cargo run --locked -- --store examples/tko list
cargo run --locked -- --store examples/tko edges demo-epic parent --incoming
```

Copy a store to a temporary directory before experimenting with mutation, so the
committed reference data remains unchanged.

## Research: a deferred FLAC inquiry

[research/schema.toml](research/schema.toml) defines research and decision
sections plus directed references and undirected related connections. Two
research nodes plan comparisons of FLAC levels 0, 5, and 8 for storage savings
and processing cost; a decision references both. The research nodes are related
reciprocally. Findings and Outcome remain empty. No collection has been selected,
no audio has been measured, and no preferred compression level is implied.

## Tko-shaped tickets

[tko/schema.toml](tko/schema.toml) maps all eight ticket types from tko revision
`373f5b16f4ebdcd610a57dec224b53230386d51b`, with canonical create scaffolds and
normalized common attributes. Five **fictional** backup-workstream tickets
illustrate an epic, research, decision, task, and bug; their fixed timestamps and
closed records are example values, not performed work.

```text
demo-bug --deps--> demo-task --deps--> demo-decision --deps--> demo-research
```

All four children name `demo-epic` as parent. Research and decision additionally
have a reciprocal `links` connection.

[TKO-MAPPING.md](TKO-MAPPING.md) records the source-cited mapping and limitations.
This is structural representation, not tko compatibility: v0 does not enforce
nonempty section/closure rules, readiness, priority bounds, or the notes protocol.
Graphiga's String restrictions and unordered sets also differ from tko's values
and ordered-list storage. Creation timestamps must be supplied explicitly.
