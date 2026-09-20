use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::document::Document;
use crate::schema::{AttributeKind, Direction, Schema, SetKind, Value};
use crate::store::NodeId;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct NodeRecord {
    pub(crate) id: String,
    #[serde(rename = "type")]
    pub(crate) node_type: String,
    pub(crate) title: String,
}

pub(crate) struct Graph<'a> {
    schema: &'a Schema,
    documents: &'a BTreeMap<NodeId, Document>,
}

impl<'a> Graph<'a> {
    pub(crate) fn new(schema: &'a Schema, documents: &'a BTreeMap<NodeId, Document>) -> Self {
        Self { schema, documents }
    }

    pub(crate) fn list(&self) -> Result<Vec<NodeRecord>> {
        self.records(self.documents.keys())
    }

    pub(crate) async fn query(&self, name: &str) -> Result<Vec<NodeRecord>> {
        let source = self.schema.query(name)?;
        let ids = crate::query::execute(self.schema, self.documents, source)
            .await
            .with_context(|| format!("execute query {name}"))?;
        self.records(ids.iter())
    }

    pub(crate) fn show(&self, id: &NodeId) -> Result<&Document> {
        self.documents
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("node not found: {id}"))
    }

    pub(crate) fn edges(
        &self,
        id: &NodeId,
        attribute_name: &str,
        incoming: bool,
    ) -> Result<Vec<NodeRecord>> {
        let attribute = self.schema.attribute(attribute_name)?;
        let direction = match &attribute.kind {
            AttributeKind::Reference(direction)
            | AttributeKind::Set(SetKind::Reference(direction)) => *direction,
            _ => bail!("attribute {attribute_name} is not a reference attribute"),
        };
        let source = self.show(id)?;
        let ids = if incoming && direction == Direction::Directed {
            self.documents
                .iter()
                .filter(|(_, document)| {
                    document
                        .reference_targets(attribute_name)
                        .contains(id.as_str())
                })
                .map(|(candidate, _)| candidate)
                .collect::<Vec<_>>()
        } else {
            source
                .reference_targets(attribute_name)
                .iter()
                .map(|target| {
                    NodeId::parse(target).with_context(|| {
                        format!(
                            "reference {attribute_name} on node {id} has invalid target {target:?}"
                        )
                    })
                })
                .collect::<Result<Vec<_>>>()?
                .iter()
                .map(|target| {
                    self.documents
                        .get_key_value(target)
                        .map(|(stored, _)| stored)
                        .ok_or_else(|| anyhow::anyhow!("reference target not found: {target}"))
                })
                .collect::<Result<Vec<_>>>()?
        };
        self.records(ids)
    }

    pub(crate) fn create(
        &self,
        node_type: &str,
        id: &NodeId,
        title: &str,
        explicit: &BTreeMap<String, String>,
        sections: &BTreeMap<String, String>,
    ) -> Result<BTreeMap<NodeId, Document>> {
        if self.documents.contains_key(id) {
            bail!("node already exists: {id}");
        }
        let type_definition = self.schema.node_type(node_type)?;
        for section in sections.keys() {
            if !type_definition.sections.contains(section) {
                bail!("node type {node_type} has no declared section {section}");
            }
        }

        let mut attributes = BTreeMap::new();
        for (name, input) in explicit {
            let value = self.schema.parse_cli_value(name, Some(input))?;
            attributes.insert(name.clone(), value);
        }
        for (name, declaration) in &self.schema.attributes {
            if declaration.required && !attributes.contains_key(name) {
                let value = declaration.default.clone().ok_or_else(|| {
                    anyhow::anyhow!(
                        "required attribute {name} needs an explicit value or schema default"
                    )
                })?;
                attributes.insert(name.clone(), value);
            }
        }
        self.validate_reference_values(id, &attributes)?;

        let document = Document::create(
            node_type.to_owned(),
            title,
            &type_definition.sections,
            sections,
            attributes,
        )?;
        let mut working = self.documents.clone();
        working.insert(id.clone(), document);
        for (name, declaration) in &self.schema.attributes {
            if is_reference(&declaration.kind) {
                let new = working[id].attributes.get(name).cloned();
                self.reconcile_reference(id, name, None, new.as_ref(), &mut working)?;
            }
        }
        Ok(changes_from(self.documents, working))
    }

    pub(crate) fn set(
        &self,
        id: &NodeId,
        attribute_name: &str,
        input: Option<&str>,
    ) -> Result<BTreeMap<NodeId, Document>> {
        let value = self.schema.parse_cli_value(attribute_name, input)?;
        self.assign(id, attribute_name, Some(value))
    }

    pub(crate) fn unset(
        &self,
        id: &NodeId,
        attribute_name: &str,
    ) -> Result<BTreeMap<NodeId, Document>> {
        let attribute = self.schema.attribute(attribute_name)?;
        if attribute.required {
            bail!("required attribute {attribute_name} cannot be unset");
        }
        self.assign(id, attribute_name, None)
    }

    pub(crate) fn add(
        &self,
        id: &NodeId,
        attribute_name: &str,
        input: &str,
    ) -> Result<BTreeMap<NodeId, Document>> {
        let members = self.schema.parse_set_members(attribute_name, input)?;
        let document = self.show(id)?;
        let mut value = match document.attributes.get(attribute_name) {
            Some(Value::Set(current)) => current.clone(),
            None => BTreeSet::new(),
            Some(_) => unreachable!("schema parsing fixes each attribute's value shape"),
        };
        value.extend(members);
        self.assign(id, attribute_name, Some(Value::Set(value)))
    }

    pub(crate) fn remove(
        &self,
        id: &NodeId,
        attribute_name: &str,
        input: &str,
    ) -> Result<BTreeMap<NodeId, Document>> {
        let members = self.schema.parse_set_members(attribute_name, input)?;
        let document = self.show(id)?;
        let mut value = match document.attributes.get(attribute_name) {
            Some(Value::Set(current)) => current.clone(),
            None => BTreeSet::new(),
            Some(_) => unreachable!("schema parsing fixes each attribute's value shape"),
        };
        value.retain(|member| !members.contains(member));
        self.assign(id, attribute_name, Some(Value::Set(value)))
    }

    fn assign(
        &self,
        id: &NodeId,
        attribute_name: &str,
        value: Option<Value>,
    ) -> Result<BTreeMap<NodeId, Document>> {
        let declaration = self.schema.attribute(attribute_name)?;
        let source = self.show(id)?;
        if let Some(value) = &value {
            self.validate_reference_value(id, &declaration.kind, value)?;
        }

        let old = source.attributes.get(attribute_name).cloned();
        let mut working = self.documents.clone();
        let source = working
            .get_mut(id)
            .expect("show established that the source exists");
        match &value {
            Some(value) => {
                source
                    .attributes
                    .insert(attribute_name.to_owned(), value.clone());
            }
            None => {
                source.attributes.remove(attribute_name);
            }
        }
        if is_reference(&declaration.kind) {
            self.reconcile_reference(
                id,
                attribute_name,
                old.as_ref(),
                value.as_ref(),
                &mut working,
            )?;
        }
        Ok(changes_from(self.documents, working))
    }

    fn validate_reference_values(
        &self,
        source: &NodeId,
        attributes: &BTreeMap<String, Value>,
    ) -> Result<()> {
        for (name, value) in attributes {
            let declaration = self.schema.attribute(name)?;
            self.validate_reference_value(source, &declaration.kind, value)?;
        }
        Ok(())
    }

    fn validate_reference_value(
        &self,
        source: &NodeId,
        kind: &AttributeKind,
        value: &Value,
    ) -> Result<()> {
        for target in reference_targets(kind, value) {
            let target = NodeId::parse(target).with_context(|| {
                format!("reference from {source} has invalid target {target:?}")
            })?;
            if &target == source {
                bail!("self-reference is not supported in v0: {source}");
            }
            if !self.documents.contains_key(&target) {
                bail!("reference target not found: {target}");
            }
        }
        Ok(())
    }

    fn reconcile_reference(
        &self,
        source: &NodeId,
        attribute_name: &str,
        old: Option<&Value>,
        new: Option<&Value>,
        working: &mut BTreeMap<NodeId, Document>,
    ) -> Result<()> {
        let declaration = self.schema.attribute(attribute_name)?;
        match &declaration.kind {
            AttributeKind::Reference(Direction::Directed)
            | AttributeKind::Set(SetKind::Reference(Direction::Directed)) => Ok(()),
            AttributeKind::Set(SetKind::Reference(Direction::Undirected)) => {
                let old = value_as_set(old);
                let new = value_as_set(new);
                for target in old.difference(&new) {
                    let target = NodeId::parse(target)?;
                    let peer = working
                        .get_mut(&target)
                        .ok_or_else(|| anyhow::anyhow!("reference target not found: {target}"))?;
                    let mut peers = value_as_set(peer.attributes.get(attribute_name));
                    peers.remove(source.as_str());
                    peer.attributes
                        .insert(attribute_name.to_owned(), Value::Set(peers));
                }
                for target in new.difference(&old) {
                    let target = NodeId::parse(target)?;
                    let peer = working
                        .get_mut(&target)
                        .ok_or_else(|| anyhow::anyhow!("reference target not found: {target}"))?;
                    let mut peers = value_as_set(peer.attributes.get(attribute_name));
                    peers.insert(source.to_string());
                    peer.attributes
                        .insert(attribute_name.to_owned(), Value::Set(peers));
                }
                Ok(())
            }
            AttributeKind::Reference(Direction::Undirected) => {
                let old = value_as_reference(old);
                let new = value_as_reference(new);
                if old == new {
                    return Ok(());
                }
                if declaration.required && old.is_some() {
                    bail!(
                        "required undirected reference {attribute_name} cannot be reassigned because it would unpair the old target"
                    );
                }
                if let Some(target) = new {
                    let target_id = NodeId::parse(target)?;
                    let target_document = working.get(&target_id).ok_or_else(|| {
                        anyhow::anyhow!("reference target not found: {target_id}")
                    })?;
                    if let Some(counterpart) =
                        value_as_reference(target_document.attributes.get(attribute_name))
                        && counterpart != source.as_str()
                    {
                        bail!(
                            "cannot pair {source} with {target_id}: target is already paired with {counterpart}"
                        );
                    }
                }
                if let Some(target) = old {
                    let target = NodeId::parse(target)?;
                    working
                        .get_mut(&target)
                        .ok_or_else(|| anyhow::anyhow!("reference target not found: {target}"))?
                        .attributes
                        .remove(attribute_name);
                }
                if let Some(target) = new {
                    let target = NodeId::parse(target)?;
                    working
                        .get_mut(&target)
                        .ok_or_else(|| anyhow::anyhow!("reference target not found: {target}"))?
                        .attributes
                        .insert(
                            attribute_name.to_owned(),
                            Value::Reference(source.to_string()),
                        );
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn records<'b>(&self, ids: impl IntoIterator<Item = &'b NodeId>) -> Result<Vec<NodeRecord>> {
        ids.into_iter()
            .map(|id| {
                let document = &self.documents[id];
                Ok(NodeRecord {
                    id: id.to_string(),
                    node_type: document.node_type.clone(),
                    title: document.title()?,
                })
            })
            .collect()
    }
}

fn is_reference(kind: &AttributeKind) -> bool {
    matches!(
        kind,
        AttributeKind::Reference(_) | AttributeKind::Set(SetKind::Reference(_))
    )
}

fn reference_targets<'a>(kind: &AttributeKind, value: &'a Value) -> Vec<&'a str> {
    match (kind, value) {
        (AttributeKind::Reference(_), Value::Reference(target)) => vec![target],
        (AttributeKind::Set(SetKind::Reference(_)), Value::Set(targets)) => {
            targets.iter().map(String::as_str).collect()
        }
        _ => Vec::new(),
    }
}

fn value_as_set(value: Option<&Value>) -> BTreeSet<String> {
    match value {
        Some(Value::Set(values)) => values.clone(),
        None => BTreeSet::new(),
        Some(_) => unreachable!("schema parsing fixes each attribute's value shape"),
    }
}

fn value_as_reference(value: Option<&Value>) -> Option<&str> {
    match value {
        Some(Value::Reference(target)) => Some(target),
        None => None,
        Some(_) => unreachable!("schema parsing fixes each attribute's value shape"),
    }
}

fn changes_from(
    original: &BTreeMap<NodeId, Document>,
    working: BTreeMap<NodeId, Document>,
) -> BTreeMap<NodeId, Document> {
    working
        .into_iter()
        .filter(|(id, document)| original.get(id) != Some(document))
        .collect()
}
