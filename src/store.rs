use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::document::Document;
use crate::schema::Schema;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct NodeId(String);

impl NodeId {
    pub(crate) fn parse(value: &str) -> Result<Self> {
        let mut characters = value.chars();
        let Some(first) = characters.next() else {
            bail!("node ID must not be empty");
        };
        if !first.is_ascii_alphanumeric()
            || !characters.all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            })
        {
            bail!(
                "node ID {value:?} must start with an ASCII letter or digit and contain only ASCII letters, digits, '.', '-', or '_'"
            );
        }
        Ok(Self(value.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    fn file_name(&self) -> String {
        format!("{}.md", self.0)
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

pub(crate) struct Store {
    root: PathBuf,
    pub(crate) schema: Schema,
    pub(crate) documents: BTreeMap<NodeId, Document>,
}

impl Store {
    pub(crate) fn load(root: &Path) -> Result<Self> {
        let schema = Schema::load(root)?;
        let entries = fs::read_dir(root)
            .with_context(|| format!("read store directory {}", root.display()))?;
        let mut documents = BTreeMap::new();
        for entry in entries {
            let entry = entry.with_context(|| format!("read entry in {}", root.display()))?;
            let file_type = entry
                .file_type()
                .with_context(|| format!("inspect {}", entry.path().display()))?;
            if !file_type.is_file() || entry.path().extension() != Some(OsStr::new("md")) {
                continue;
            }
            let file_name = entry.file_name().into_string().map_err(|_| {
                anyhow::anyhow!("node filename is not UTF-8: {}", entry.path().display())
            })?;
            let id_text = file_name
                .strip_suffix(".md")
                .expect("extension check established the suffix");
            let id = NodeId::parse(id_text)
                .with_context(|| format!("derive node ID from {file_name}"))?;
            let text = fs::read_to_string(entry.path())
                .with_context(|| format!("read node {}", entry.path().display()))?;
            let document = Document::parse(&text, &schema)
                .with_context(|| format!("parse node {}", entry.path().display()))?;
            documents.insert(id, document);
        }
        Ok(Self {
            root: root.to_owned(),
            schema,
            documents,
        })
    }

    /// Persists a fully calculated change set in ID order.
    ///
    /// Rendering is completed before the first write. An I/O error can still occur
    /// after earlier files were written or after changing part of the failing file;
    /// the returned error names known progress and no rollback is attempted.
    pub(crate) fn persist(&self, changes: &BTreeMap<NodeId, Document>) -> Result<()> {
        let mut prepared = Vec::with_capacity(changes.len());
        for (id, document) in changes {
            let text = document
                .render()
                .with_context(|| format!("prepare node {id} for writing"))?;
            prepared.push((id, self.root.join(id.file_name()), text));
        }

        let mut written = Vec::new();
        for (id, path, text) in prepared {
            if let Err(source) = fs::write(&path, text) {
                let progress = if written.is_empty() {
                    "no earlier node write completed; the failing file may be partial; no rollback attempted"
                        .to_owned()
                } else {
                    format!(
                        "completed earlier writes to {}; the failing file may be partial; no rollback attempted",
                        written.join(", ")
                    )
                };
                return Err(source).with_context(|| {
                    format!("write node {id} at {} ({progress})", path.display())
                });
            }
            written.push(id.to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::schema::Value;

    #[test]
    fn write_error_reports_known_partial_progress_without_rollback() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("schema.toml"),
            "version = 0\n[attributes.status]\ntype = \"enum\"\nvalues = [\"open\", \"done\"]\nrequired = true\ndefault = \"open\"\n[types.item]\nsections = []\n",
        )
        .unwrap();
        for id in ["a", "b"] {
            fs::write(
                directory.path().join(format!("{id}.md")),
                "+++\ntype = \"item\"\nstatus = \"open\"\n+++\n\n# Title\n",
            )
            .unwrap();
        }

        let store = Store::load(directory.path()).unwrap();
        let mut changes = store.documents.clone();
        for document in changes.values_mut() {
            document
                .attributes
                .insert("status".to_owned(), Value::Enum("done".to_owned()));
        }
        fs::remove_file(directory.path().join("b.md")).unwrap();
        fs::create_dir(directory.path().join("b.md")).unwrap();

        let error = store.persist(&changes).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("write node b"));
        assert!(message.contains("completed earlier writes to a"));
        assert!(message.contains("the failing file may be partial; no rollback attempted"));
        assert!(
            fs::read_to_string(directory.path().join("a.md"))
                .unwrap()
                .contains("status = \"done\"")
        );
    }
}
