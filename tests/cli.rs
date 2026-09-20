use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const SPECIMEN_SCHEMA: &str = r#"version = 0

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
"#;

const STORAGE: &str = r#"+++
type = "research"
status = "open"
reviewed = false
tags = ["audio", "flac"]
related = ["processing-cost"]
+++

# How does FLAC compression level affect storage size?

## Question

Question body.

## Investigation

Investigation body.

## Findings
"#;

const PROCESSING: &str = r#"+++
type = "research"
status = "open"
reviewed = false
tags = ["audio", "flac"]
related = ["storage-savings"]
+++

# What processing cost do FLAC levels 0, 5, and 8 impose?

## Question

Question body.

## Investigation

Investigation body.

## Findings
"#;

const DECISION: &str = r#"+++
type = "decision"
status = "open"
reviewed = false
references = ["storage-savings", "processing-cost"]
+++

# Which FLAC compression level should we use for this collection?

## Question

Question body.

## Outcome
"#;

fn graphiga(store: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_graphiga"))
        .arg("--store")
        .arg(store)
        .args(arguments)
        .output()
        .expect("run graphiga")
}

fn success(store: &Path, arguments: &[&str]) -> String {
    let output = graphiga(store, arguments);
    assert!(
        output.status.success(),
        "command {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 stdout")
}

fn specimen() -> TempDir {
    let directory = tempfile::tempdir().expect("temporary store");
    fs::write(directory.path().join("schema.toml"), SPECIMEN_SCHEMA).unwrap();
    fs::write(directory.path().join("storage-savings.md"), STORAGE).unwrap();
    fs::write(directory.path().join("processing-cost.md"), PROCESSING).unwrap();
    fs::write(directory.path().join("compression-choice.md"), DECISION).unwrap();
    directory
}

fn body(document: &[u8]) -> &[u8] {
    let marker = b"\n+++\n";
    let start = document
        .windows(marker.len())
        .position(|window| window == marker)
        .expect("closing delimiter")
        + marker.len();
    &document[start..]
}

#[test]
fn reserved_structural_attribute_names_are_rejected_before_writes() {
    for name in ["type", "id", "title"] {
        let store = tempfile::tempdir().unwrap();
        fs::write(
            store.path().join("schema.toml"),
            format!(
                "version = 0\n[attributes.{name}]\ntype = \"string\"\n[types.item]\nsections = []\n"
            ),
        )
        .unwrap();

        let output = graphiga(
            store.path(),
            &[
                "create", "item", "node", "--title", "Title", "--set", name, "value",
            ],
        );
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("reserved"));
        assert!(!store.path().join("node.md").exists());
    }
}

#[test]
fn handwritten_specimen_lists_and_exposes_expected_edges() {
    let store = specimen();
    let human = success(store.path(), &["list"]);
    let json: Value = serde_json::from_str(&success(store.path(), &["list", "--json"])).unwrap();
    let ids: Vec<_> = json
        .as_array()
        .unwrap()
        .iter()
        .map(|record| record["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        ["compression-choice", "processing-cost", "storage-savings"]
    );
    for id in &ids {
        assert!(human.contains(id));
    }

    let outgoing = success(
        store.path(),
        &["edges", "compression-choice", "references", "--json"],
    );
    let outgoing: Value = serde_json::from_str(&outgoing).unwrap();
    assert_eq!(outgoing.as_array().unwrap().len(), 2);
    let incoming = success(
        store.path(),
        &[
            "edges",
            "storage-savings",
            "references",
            "--incoming",
            "--json",
        ],
    );
    let incoming: Value = serde_json::from_str(&incoming).unwrap();
    assert_eq!(incoming[0]["id"], "compression-choice");
    let related = success(
        store.path(),
        &[
            "edges",
            "storage-savings",
            "related",
            "--incoming",
            "--json",
        ],
    );
    let related: Value = serde_json::from_str(&related).unwrap();
    assert_eq!(related[0]["id"], "processing-cost");
}

#[test]
fn create_uses_defaults_overrides_sections_and_reciprocal_references() {
    let store = tempfile::tempdir().unwrap();
    fs::write(store.path().join("schema.toml"), SPECIMEN_SCHEMA).unwrap();

    success(
        store.path(),
        &[
            "create",
            "research",
            "first",
            "--title",
            "First title",
            "--set",
            "status",
            "done",
            "--set",
            "tags",
            "z,a,a",
            "--section",
            "Question",
            "A question",
            "--section",
            "Investigation",
            "Plan\nwith detail",
        ],
    );
    success(
        store.path(),
        &[
            "create",
            "research",
            "second",
            "--title",
            "Second title",
            "--set",
            "related",
            "first",
        ],
    );
    success(
        store.path(),
        &[
            "create",
            "decision",
            "choice",
            "--title",
            "Choose one",
            "--set",
            "references",
            "first,second",
            "--section",
            "Question",
            "Which one?",
        ],
    );

    let first = fs::read_to_string(store.path().join("first.md")).unwrap();
    assert!(first.contains("status = \"done\""));
    assert!(first.contains("reviewed = false"));
    assert!(first.contains("tags = [\"a\", \"z\"]"));
    assert!(first.contains("## Question\n\nA question"));
    assert!(first.contains("## Investigation\n\nPlan\nwith detail"));
    assert!(first.contains("## Findings"));
    assert!(first.contains("related = [\"second\"]"));
    assert!(!first.contains("references ="));
    let choice = fs::read_to_string(store.path().join("choice.md")).unwrap();
    assert!(choice.contains("references = [\"first\", \"second\"]"));
    assert!(choice.contains("## Outcome"));

    let first_before = fs::read(store.path().join("first.md")).unwrap();
    let overwrite = graphiga(
        store.path(),
        &["create", "research", "first", "--title", "Replacement"],
    );
    assert!(!overwrite.status.success());
    assert_eq!(
        fs::read(store.path().join("first.md")).unwrap(),
        first_before
    );

    let duplicate = graphiga(
        store.path(),
        &[
            "create", "decision", "bad", "--title", "Bad", "--set", "status", "open", "--set",
            "status", "done",
        ],
    );
    assert!(!duplicate.status.success());
    assert!(!store.path().join("bad.md").exists());
}

#[test]
fn show_supports_all_modes_and_ignores_fenced_heading_text() {
    let store = specimen();
    let path = store.path().join("storage-savings.md");
    let text = fs::read_to_string(&path).unwrap().replace(
        "Investigation body.",
        "Investigation body.\n\n### Detail\n\n```text\n## Not a section\n### Not a heading\n```",
    );
    fs::write(&path, &text).unwrap();

    let outline = success(store.path(), &["show", "storage-savings"]);
    assert!(outline.contains("# How does FLAC"));
    assert!(outline.contains("### Detail"));
    assert!(!outline.contains("Not a heading"));

    let section_outline = success(
        store.path(),
        &["show", "storage-savings", "--section", "Investigation"],
    );
    assert_eq!(section_outline, "## Investigation\n### Detail\n");

    let section_full = success(
        store.path(),
        &[
            "show",
            "storage-savings",
            "--section",
            "Investigation",
            "--full",
        ],
    );
    assert!(section_full.starts_with("## Investigation"));
    assert!(section_full.contains("## Not a section"));
    assert!(!section_full.contains("## Findings"));

    assert_eq!(
        success(store.path(), &["show", "storage-savings", "--full"]),
        text
    );

    fs::write(&path, text.replace("\n## Findings\n", "")).unwrap();
    let missing = graphiga(
        store.path(),
        &["show", "storage-savings", "--section", "Findings", "--full"],
    );
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("section not found"));
}

#[test]
fn set_operations_are_sets_and_preserve_body_bytes() {
    let store = specimen();
    let path = store.path().join("storage-savings.md");
    let original = fs::read(&path).unwrap();

    success(store.path(), &["set", "storage-savings", "tags", "c,a,c"]);
    success(store.path(), &["add", "storage-savings", "tags", "b,a"]);
    success(store.path(), &["remove", "storage-savings", "tags", "c,c"]);
    let changed = fs::read(&path).unwrap();
    assert_eq!(body(&changed), body(&original));
    let changed_text = String::from_utf8(changed).unwrap();
    assert!(changed_text.contains("tags = [\"a\", \"b\"]"));

    success(store.path(), &["set", "storage-savings", "tags"]);
    let empty = fs::read_to_string(&path).unwrap();
    assert!(empty.contains("tags = []"));
    success(store.path(), &["unset", "storage-savings", "tags"]);
    let absent = fs::read_to_string(&path).unwrap();
    assert!(!absent.contains("tags ="));
    success(store.path(), &["remove", "storage-savings", "tags", "a"]);
    assert!(fs::read_to_string(&path).unwrap().contains("tags = []"));

    let before = fs::read(&path).unwrap();
    let bad = graphiga(store.path(), &["set", "storage-savings", "reviewed", "yes"]);
    assert!(!bad.status.success());
    assert_eq!(fs::read(&path).unwrap(), before);
    let required = graphiga(store.path(), &["unset", "storage-savings", "status"]);
    assert!(!required.status.success());
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn directed_and_undirected_mutations_update_only_required_files() {
    let store = specimen();
    let storage_path = store.path().join("storage-savings.md");
    let processing_path = store.path().join("processing-cost.md");
    let decision_path = store.path().join("compression-choice.md");
    let bodies = [
        body(&fs::read(&storage_path).unwrap()).to_vec(),
        body(&fs::read(&processing_path).unwrap()).to_vec(),
        body(&fs::read(&decision_path).unwrap()).to_vec(),
    ];

    success(
        store.path(),
        &["remove", "storage-savings", "related", "processing-cost"],
    );
    assert!(
        !fs::read_to_string(&storage_path)
            .unwrap()
            .contains("processing-cost")
    );
    assert!(
        !fs::read_to_string(&processing_path)
            .unwrap()
            .contains("storage-savings")
    );
    success(
        store.path(),
        &[
            "add",
            "storage-savings",
            "related",
            "processing-cost,processing-cost",
        ],
    );
    assert!(
        fs::read_to_string(&storage_path)
            .unwrap()
            .contains("processing-cost")
    );
    assert!(
        fs::read_to_string(&processing_path)
            .unwrap()
            .contains("storage-savings")
    );

    let processing_before = fs::read(&processing_path).unwrap();
    success(
        store.path(),
        &[
            "remove",
            "compression-choice",
            "references",
            "processing-cost",
        ],
    );
    assert_eq!(fs::read(&processing_path).unwrap(), processing_before);
    assert_eq!(body(&fs::read(&storage_path).unwrap()), bodies[0]);
    assert_eq!(body(&fs::read(&processing_path).unwrap()), bodies[1]);
    assert_eq!(body(&fs::read(&decision_path).unwrap()), bodies[2]);

    success(store.path(), &["set", "storage-savings", "related"]);
    assert!(
        fs::read_to_string(&storage_path)
            .unwrap()
            .contains("related = []")
    );
    assert!(
        fs::read_to_string(&processing_path)
            .unwrap()
            .contains("related = []")
    );
    success(store.path(), &["unset", "storage-savings", "related"]);
    assert!(
        !fs::read_to_string(&storage_path)
            .unwrap()
            .contains("related =")
    );

    let snapshot = [
        fs::read(&storage_path).unwrap(),
        fs::read(&processing_path).unwrap(),
        fs::read(&decision_path).unwrap(),
    ];
    let missing = graphiga(
        store.path(),
        &["add", "storage-savings", "related", "does-not-exist"],
    );
    assert!(!missing.status.success());
    assert_eq!(fs::read(&storage_path).unwrap(), snapshot[0]);
    assert_eq!(fs::read(&processing_path).unwrap(), snapshot[1]);
    assert_eq!(fs::read(&decision_path).unwrap(), snapshot[2]);
}

#[test]
fn supplementary_types_and_exclusive_pairing_follow_schema() {
    let store = tempfile::tempdir().unwrap();
    fs::write(
        store.path().join("schema.toml"),
        r#"version = 0
[attributes.label]
type = "string"
[attributes.count]
type = "integer"
[attributes.mode]
type = "enum"
values = ["one", "two"]
[attributes.partner]
type = "reference"
direction = "undirected"
[types.item]
sections = []
"#,
    )
    .unwrap();
    for id in ["a", "b", "c"] {
        success(
            store.path(),
            &["create", "item", id, "--title", &format!("Node {id}")],
        );
    }
    success(store.path(), &["set", "a", "label", "-token"]);
    success(store.path(), &["set", "a", "count", "-7"]);
    success(store.path(), &["set", "a", "mode", "two"]);
    success(store.path(), &["set", "b", "partner", "c"]);

    let before: Vec<_> = ["a", "b", "c"]
        .iter()
        .map(|id| fs::read(store.path().join(format!("{id}.md"))).unwrap())
        .collect();
    let conflict = graphiga(store.path(), &["set", "a", "partner", "b"]);
    assert!(!conflict.status.success());
    for (index, id) in ["a", "b", "c"].iter().enumerate() {
        assert_eq!(
            fs::read(store.path().join(format!("{id}.md"))).unwrap(),
            before[index]
        );
    }

    success(store.path(), &["set", "b", "partner", "a"]);
    let a = fs::read_to_string(store.path().join("a.md")).unwrap();
    let b = fs::read_to_string(store.path().join("b.md")).unwrap();
    let c = fs::read_to_string(store.path().join("c.md")).unwrap();
    assert!(a.contains("partner = \"b\""));
    assert!(b.contains("partner = \"a\""));
    assert!(!c.contains("partner ="));
}
