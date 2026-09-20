use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};

use crate::document::Document;
use crate::schema::{AttributeKind, Direction, Schema, SetKind};
use crate::store::NodeId;

/// A schema attribute parsed as a reference relationship.
#[derive(Debug)]
pub(crate) struct ReferenceAttribute {
    name: String,
    direction: Direction,
}

impl ReferenceAttribute {
    pub(crate) fn parse(name: &str, schema: &Schema) -> Result<Self> {
        let direction = match schema.attribute(name)?.kind {
            AttributeKind::Reference(direction)
            | AttributeKind::Set(SetKind::Reference(direction)) => direction,
            _ => bail!("field {name} is not a reference field"),
        };
        Ok(Self {
            name: name.to_owned(),
            direction,
        })
    }

    /// Returns adjacent IDs under the schema's direction policy.
    ///
    /// Directed incoming lookup scans declarations. Outgoing and undirected
    /// lookup use the current document's declarations; v0 assumes undirected
    /// declarations are reciprocal. Every returned target is parsed and must
    /// identify a loaded document.
    pub(crate) fn neighbor_ids(
        &self,
        id: &NodeId,
        document: &Document,
        documents: &BTreeMap<NodeId, Document>,
        incoming: bool,
    ) -> Result<BTreeSet<NodeId>> {
        if incoming && self.direction == Direction::Directed {
            return Ok(documents
                .iter()
                .filter(|(_, candidate)| {
                    candidate
                        .reference_targets(&self.name)
                        .contains(id.as_str())
                })
                .map(|(candidate_id, _)| candidate_id.clone())
                .collect());
        }

        document
            .reference_targets(&self.name)
            .into_iter()
            .map(|target| {
                let target_id = NodeId::parse(&target).with_context(|| {
                    format!(
                        "reference {} on node {id} has invalid target {target:?}",
                        self.name
                    )
                })?;
                if !documents.contains_key(&target_id) {
                    bail!("reference target not found: {target_id}");
                }
                Ok(target_id)
            })
            .collect()
    }
}
