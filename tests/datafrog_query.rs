use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const SCHEMA: &str = r#"version = 0

[attributes.enabled]
type = "boolean"

[attributes.count]
type = "integer"

[attributes.blocked]
type = "boolean"

[attributes.parent]
type = "reference"
direction = "directed"

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

fn store_with_query(program: &str) -> TempDir {
    let directory = tempfile::tempdir().expect("temporary store");
    let schema = format!("{SCHEMA}\n[queries]\ntest = {program:?}\n");
    fs::write(directory.path().join("schema.toml"), schema).unwrap();
    directory
}

fn write_node(store: &Path, id: &str, frontmatter: &str) {
    fs::write(
        store.join(format!("{id}.md")),
        format!("+++\ntype = \"item\"\n{frontmatter}+++\n\n# Node {id}\n"),
    )
    .unwrap();
}

#[test]
fn grammar_evaluates_typed_filters_recursion_union_and_stratified_negation() {
    let program = r#"
ancestor(N, A) :- edge(N, "parent", A).
ancestor(N, A) :- edge(N, "parent", P), ancestor(P, A).
blocked(N) :- attr(N, "blocked", true).
selected(N) :- attr(N, "enabled", true), attr(N, "count", Count), Count = 2.
candidate(N) :- ancestor(N, "a"), N != "a".
always("a") :- true = true.
result(N) :- selected(N).
result(N) :- candidate(N), not blocked(N).
result(N) :- always(N).
"#;
    let store = store_with_query(program);
    write_node(store.path(), "a", "enabled = true\ncount = 2\n");
    write_node(store.path(), "b", "parent = \"a\"\n");
    write_node(store.path(), "c", "parent = \"b\"\nblocked = true\n");

    let output = graphiga(store.path(), &["query", "test", "--json"]);
    assert!(
        output.status.success(),
        "query failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows: Value = serde_json::from_slice(&output.stdout).unwrap();
    let ids: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["a", "b"]);
}

#[test]
fn grammar_and_binding_fail_before_scanning_an_empty_store() {
    let invalid = [
        ("result(N) :-", "syntax error"),
        (
            "result(N) :- node(\"a\", \"item\", \"A\").",
            "unsafe head variable N",
        ),
        (
            "result(\"a\") :- node(\"a\", \"item\", \"A\"), not edge(N, \"parent\", \"a\").",
            "unsafe negated atom variable N",
        ),
        (
            "result(\"a\") :- node(\"a\", \"item\", \"A\"), N = 2.",
            "unsafe filter variable N",
        ),
        (
            "helper(N) :- node(N, \"item\", \"A\"). result(N) :- helper(N, \"extra\").",
            "used with arities 1 and 2",
        ),
        (
            "node(N, T, Title) :- attr(N, \"kind\", T), node(N, T, Title). result(N) :- node(N, T, Title).",
            "base relation node is read-only",
        ),
        (
            "left(N) :- node(N, T, Title), not right(N). right(N) :- node(N, T, Title), not left(N). result(N) :- left(N).",
            "recursive negation",
        ),
    ];

    for (program, diagnostic) in invalid {
        let store = store_with_query(program);
        let output = graphiga(store.path(), &["query", "test", "--json"]);
        assert!(
            !output.status.success(),
            "invalid program succeeded: {program}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(diagnostic),
            "expected {diagnostic:?} for {program:?}, got {stderr:?}"
        );
    }
}

#[test]
fn base_negation_is_a_valid_lower_stratum() {
    let store =
        store_with_query("result(N) :- node(N, \"item\", Title), not attr(N, \"blocked\", true).");
    write_node(store.path(), "a", "");
    write_node(store.path(), "b", "blocked = true\n");

    let output = graphiga(store.path(), &["query", "test", "--json"]);
    assert!(
        output.status.success(),
        "query failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rows[0]["id"], "a");
    assert_eq!(rows.as_array().unwrap().len(), 1);
}
