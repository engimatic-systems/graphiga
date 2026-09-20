use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const SCHEMA: &str = r#"version = 0

[queries]
precedence = "state = open or count >= 10 and not tags contain hidden"
grouped = "(state = open or count >= 10) and not tags contain hidden"
all_comparisons = "count = 0 and count != 1 and count < 1 and count <= 0 and count > -1 and count >= 0"
membership = "state in [open]"
presence = "has flag and flag = false and has count and count = 0 and no label and no deps"
structural = "id = a and type = item and title = Alpha"
any_dependency = "any deps (state = open and count = 0)"
any_parent = "any parent (id = a)"
dependency_reaches = "id != a and reaches deps a"
descendants = "id != a and reaches parent a"
undirected_any = "any peer (id = c)"
undirected_reaches = "id != a and reaches peer a"

[attributes.state]
type = "enum"
values = ["open", "closed"]
required = true

[attributes.flag]
type = "boolean"

[attributes.count]
type = "integer"

[attributes.label]
type = "string"

[attributes.tags]
type = "set"
items = "string"

[attributes.deps]
type = "set"
items = "reference"
direction = "directed"

[attributes.parent]
type = "reference"
direction = "directed"

[attributes.peer]
type = "reference"
direction = "undirected"

[types.item]
sections = []
"#;

fn graphiga(store: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_graphiga"))
        .arg("--store")
        .arg(store)
        .args(arguments)
        .output()
        .expect("run graphiga")
}

fn query_ids(store: &Path, name: &str) -> Vec<String> {
    let output = graphiga(store, &["query", name, "--json"]);
    assert!(
        output.status.success(),
        "query {name} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows: Value = serde_json::from_slice(&output.stdout).expect("JSON query output");
    rows.as_array()
        .expect("array output")
        .iter()
        .map(|row| row["id"].as_str().expect("ID string").to_owned())
        .collect()
}

fn query_store() -> TempDir {
    let store = tempfile::tempdir().expect("temporary store");
    fs::write(store.path().join("schema.toml"), SCHEMA).unwrap();
    fs::write(
        store.path().join("a.md"),
        r#"+++
type = "item"
state = "open"
flag = false
count = 0
tags = ["hidden"]
deps = []
parent = "c"
peer = "c"
+++
# Alpha
"#,
    )
    .unwrap();
    fs::write(
        store.path().join("b.md"),
        r#"+++
type = "item"
state = "closed"
count = 10
tags = ["visible"]
deps = ["a"]
parent = "a"
+++
# Beta
"#,
    )
    .unwrap();
    fs::write(
        store.path().join("c.md"),
        r#"+++
type = "item"
state = "closed"
count = 9
tags = ["visible"]
deps = ["b"]
parent = "b"
peer = "a"
+++
# Gamma
"#,
    )
    .unwrap();
    store
}

#[test]
fn local_grammar_is_typed_and_obeys_not_and_or_precedence() {
    let store = query_store();
    assert_eq!(query_ids(store.path(), "precedence"), ["a", "b"]);
    assert_eq!(query_ids(store.path(), "grouped"), ["b"]);
    assert_eq!(query_ids(store.path(), "all_comparisons"), ["a"]);
    assert_eq!(query_ids(store.path(), "membership"), ["a"]);
    assert_eq!(query_ids(store.path(), "presence"), ["a"]);
    assert_eq!(query_ids(store.path(), "structural"), ["a"]);
}

#[test]
fn graph_operators_follow_schema_references_and_terminate_on_cycles() {
    let store = query_store();
    assert_eq!(query_ids(store.path(), "any_dependency"), ["b"]);
    assert_eq!(query_ids(store.path(), "any_parent"), ["b"]);
    assert_eq!(query_ids(store.path(), "dependency_reaches"), ["b", "c"]);
    assert_eq!(query_ids(store.path(), "descendants"), ["b", "c"]);
    assert_eq!(query_ids(store.path(), "undirected_any"), ["a"]);
    assert_eq!(query_ids(store.path(), "undirected_reaches"), ["c"]);
}

#[test]
fn selected_queries_are_validated_before_an_empty_store_is_scanned() {
    let store = tempfile::tempdir().unwrap();
    fs::write(
        store.path().join("schema.toml"),
        r#"version = 0
[queries]
syntax = "state ="
unknown_field = "missing = open"
set_comparison = "tags = hidden"
scalar_containment = "state contain open"
any_non_reference = "any tags (state = open)"
reaches_non_reference = "reaches state a"
bad_enum = "state = invented"
bad_boolean = "flag = zero"
bad_integer = "count = zero"
bad_reference = "parent = bad/id"
[attributes.state]
type = "enum"
values = ["open", "closed"]
[attributes.flag]
type = "boolean"
[attributes.count]
type = "integer"
[attributes.tags]
type = "set"
items = "string"
[attributes.parent]
type = "reference"
direction = "directed"
[types.item]
sections = []
"#,
    )
    .unwrap();

    for name in [
        "syntax",
        "unknown_field",
        "set_comparison",
        "scalar_containment",
        "any_non_reference",
        "reaches_non_reference",
        "bad_enum",
        "bad_boolean",
        "bad_integer",
        "bad_reference",
        "not_declared",
    ] {
        let output = graphiga(store.path(), &["query", name, "--json"]);
        assert!(
            !output.status.success(),
            "invalid query {name} unexpectedly succeeded"
        );
        assert!(
            !output.stderr.is_empty(),
            "invalid query {name} had no diagnostic"
        );
    }
}
