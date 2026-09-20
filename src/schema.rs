use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

pub(crate) const SCHEMA_FILE_NAME: &str = "schema.toml";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Direction {
    Directed,
    Undirected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SetKind {
    String,
    Reference(Direction),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AttributeKind {
    String,
    Boolean,
    Integer,
    Enum(BTreeSet<String>),
    Reference(Direction),
    Set(SetKind),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Value {
    String(String),
    Boolean(bool),
    Integer(i64),
    Enum(String),
    Reference(String),
    Set(BTreeSet<String>),
}

#[derive(Clone, Debug)]
pub(crate) struct Attribute {
    pub(crate) kind: AttributeKind,
    pub(crate) required: bool,
    pub(crate) default: Option<Value>,
}

#[derive(Clone, Debug)]
pub(crate) struct NodeType {
    pub(crate) sections: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct Schema {
    pub(crate) attributes: BTreeMap<String, Attribute>,
    pub(crate) types: BTreeMap<String, NodeType>,
    queries: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSchema {
    version: u64,
    #[serde(default)]
    attributes: BTreeMap<String, RawAttribute>,
    #[serde(default)]
    types: BTreeMap<String, RawNodeType>,
    #[serde(default)]
    queries: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAttribute {
    #[serde(rename = "type")]
    type_name: String,
    items: Option<String>,
    direction: Option<String>,
    values: Option<Vec<String>>,
    #[serde(default)]
    required: bool,
    default: Option<toml::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNodeType {
    #[serde(default)]
    sections: Vec<String>,
}

impl Schema {
    pub(crate) fn load(root: &Path) -> Result<Self> {
        let path = root.join(SCHEMA_FILE_NAME);
        let text =
            fs::read_to_string(&path).with_context(|| format!("read schema {}", path.display()))?;
        let raw: RawSchema = toml::from_str(&text)
            .with_context(|| format!("parse TOML schema {}", path.display()))?;
        Self::parse(raw).with_context(|| format!("interpret schema {}", path.display()))
    }

    fn parse(raw: RawSchema) -> Result<Self> {
        if raw.version != 0 {
            bail!("unsupported schema version {}; expected 0", raw.version);
        }

        let mut attributes = BTreeMap::new();
        for (name, raw_attribute) in raw.attributes {
            validate_token("attribute name", &name)?;
            if matches!(name.as_str(), "type" | "id" | "title") {
                bail!("attribute name {name} is reserved for document structure");
            }
            let kind = parse_kind(&name, &raw_attribute)?;
            let default = raw_attribute
                .default
                .as_ref()
                .map(|value| value_from_toml(&name, &kind, value))
                .transpose()?;
            attributes.insert(
                name,
                Attribute {
                    kind,
                    required: raw_attribute.required,
                    default,
                },
            );
        }

        let mut types = BTreeMap::new();
        for (name, raw_type) in raw.types {
            validate_token("node type", &name)?;
            let mut seen = BTreeSet::new();
            for section in &raw_type.sections {
                if section.is_empty() || section.contains(['\r', '\n']) {
                    bail!("node type {name} has an invalid section heading");
                }
                if !seen.insert(section.clone()) {
                    bail!("node type {name} repeats section {section}");
                }
            }
            types.insert(
                name,
                NodeType {
                    sections: raw_type.sections,
                },
            );
        }

        Ok(Self {
            attributes,
            types,
            queries: raw.queries,
        })
    }

    pub(crate) fn parse_cli_value(&self, name: &str, input: Option<&str>) -> Result<Value> {
        let attribute = self.attribute(name)?;
        match &attribute.kind {
            AttributeKind::Set(kind) => {
                let members = parse_set_input(name, kind, input.unwrap_or(""))?;
                Ok(Value::Set(members))
            }
            kind => {
                let input =
                    input.ok_or_else(|| anyhow::anyhow!("attribute {name} requires a value"))?;
                value_from_str(name, kind, input)
            }
        }
    }

    pub(crate) fn parse_set_members(&self, name: &str, input: &str) -> Result<BTreeSet<String>> {
        let attribute = self.attribute(name)?;
        let AttributeKind::Set(kind) = &attribute.kind else {
            bail!("attribute {name} is not a set");
        };
        parse_set_input(name, kind, input)
    }

    pub(crate) fn parse_scalar_literal(&self, name: &str, input: &str) -> Result<Value> {
        let attribute = self.attribute(name)?;
        if matches!(attribute.kind, AttributeKind::Set(_)) {
            bail!("attribute {name} is not scalar");
        }
        value_from_str(name, &attribute.kind, input)
    }

    pub(crate) fn attribute(&self, name: &str) -> Result<&Attribute> {
        self.attributes
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("unknown attribute {name}"))
    }

    pub(crate) fn node_type(&self, name: &str) -> Result<&NodeType> {
        self.types
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("unknown node type {name}"))
    }

    pub(crate) fn query(&self, name: &str) -> Result<&str> {
        self.queries
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| anyhow::anyhow!("unknown query {name}"))
    }

    pub(crate) fn value_from_toml(&self, name: &str, value: &toml::Value) -> Result<Value> {
        let attribute = self.attribute(name)?;
        value_from_toml(name, &attribute.kind, value)
    }
}

impl Value {
    pub(crate) fn to_toml(&self) -> toml::Value {
        match self {
            Self::String(value) | Self::Enum(value) | Self::Reference(value) => {
                toml::Value::String(value.clone())
            }
            Self::Boolean(value) => toml::Value::Boolean(*value),
            Self::Integer(value) => toml::Value::Integer(*value),
            Self::Set(values) => toml::Value::Array(
                values
                    .iter()
                    .map(|value| toml::Value::String(value.clone()))
                    .collect(),
            ),
        }
    }
}

fn parse_kind(name: &str, raw: &RawAttribute) -> Result<AttributeKind> {
    let direction = raw
        .direction
        .as_deref()
        .map(|value| match value {
            "directed" => Ok(Direction::Directed),
            "undirected" => Ok(Direction::Undirected),
            other => bail!("attribute {name} has unknown direction {other}"),
        })
        .transpose()?;

    let kind = match raw.type_name.as_str() {
        "string" => AttributeKind::String,
        "boolean" => AttributeKind::Boolean,
        "integer" => AttributeKind::Integer,
        "enum" => {
            let values = raw
                .values
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("enum attribute {name} requires values"))?;
            let values: BTreeSet<_> = values.iter().cloned().collect();
            if values.is_empty() {
                bail!("enum attribute {name} requires at least one value");
            }
            AttributeKind::Enum(values)
        }
        "reference" => AttributeKind::Reference(direction.ok_or_else(|| {
            anyhow::anyhow!("reference attribute {name} requires explicit direction")
        })?),
        "set" => match raw.items.as_deref() {
            Some("string") if direction.is_none() => AttributeKind::Set(SetKind::String),
            Some("string") => bail!("string set attribute {name} cannot declare direction"),
            Some("reference") => {
                AttributeKind::Set(SetKind::Reference(direction.ok_or_else(|| {
                    anyhow::anyhow!("reference set attribute {name} requires explicit direction")
                })?))
            }
            Some(other) => bail!("set attribute {name} has unsupported item type {other}"),
            None => bail!("set attribute {name} requires items"),
        },
        other => bail!("attribute {name} has unsupported type {other}"),
    };

    if !matches!(kind, AttributeKind::Reference(_) | AttributeKind::Set(_))
        && raw.direction.is_some()
    {
        bail!("non-reference attribute {name} cannot declare direction");
    }
    if !matches!(kind, AttributeKind::Set(_)) && raw.items.is_some() {
        bail!("non-set attribute {name} cannot declare items");
    }
    if !matches!(kind, AttributeKind::Enum(_)) && raw.values.is_some() {
        bail!("non-enum attribute {name} cannot declare values");
    }
    Ok(kind)
}

fn value_from_str(name: &str, kind: &AttributeKind, input: &str) -> Result<Value> {
    match kind {
        AttributeKind::String => {
            validate_string(name, input)?;
            Ok(Value::String(input.to_owned()))
        }
        AttributeKind::Boolean => match input {
            "true" => Ok(Value::Boolean(true)),
            "false" => Ok(Value::Boolean(false)),
            _ => bail!("attribute {name} expects true or false"),
        },
        AttributeKind::Integer => input
            .parse::<i64>()
            .map(Value::Integer)
            .with_context(|| format!("attribute {name} expects a signed 64-bit integer")),
        AttributeKind::Enum(values) => {
            if !values.contains(input) {
                bail!(
                    "attribute {name} expects one of: {}",
                    values.iter().cloned().collect::<Vec<_>>().join(", ")
                );
            }
            Ok(Value::Enum(input.to_owned()))
        }
        AttributeKind::Reference(_) => Ok(Value::Reference(input.to_owned())),
        AttributeKind::Set(_) => unreachable!("sets are parsed by parse_set_input"),
    }
}

fn value_from_toml(name: &str, kind: &AttributeKind, value: &toml::Value) -> Result<Value> {
    match (kind, value) {
        (AttributeKind::String, toml::Value::String(value)) => {
            validate_string(name, value)?;
            Ok(Value::String(value.clone()))
        }
        (AttributeKind::Boolean, toml::Value::Boolean(value)) => Ok(Value::Boolean(*value)),
        (AttributeKind::Integer, toml::Value::Integer(value)) => Ok(Value::Integer(*value)),
        (AttributeKind::Enum(values), toml::Value::String(value)) if values.contains(value) => {
            Ok(Value::Enum(value.clone()))
        }
        (AttributeKind::Reference(_), toml::Value::String(value)) => {
            Ok(Value::Reference(value.clone()))
        }
        (AttributeKind::Set(kind), toml::Value::Array(values)) => {
            let mut result = BTreeSet::new();
            for value in values {
                let toml::Value::String(value) = value else {
                    bail!("set attribute {name} contains a non-string member");
                };
                validate_set_member(name, kind, value)?;
                result.insert(value.clone());
            }
            Ok(Value::Set(result))
        }
        _ => bail!("attribute {name} has the wrong TOML value type"),
    }
}

fn parse_set_input(name: &str, kind: &SetKind, input: &str) -> Result<BTreeSet<String>> {
    if input.is_empty() {
        return Ok(BTreeSet::new());
    }
    let mut values = BTreeSet::new();
    for value in input.split(',') {
        if value.is_empty() {
            bail!("set attribute {name} contains an empty member");
        }
        validate_set_member(name, kind, value)?;
        values.insert(value.to_owned());
    }
    Ok(values)
}

fn validate_set_member(name: &str, kind: &SetKind, value: &str) -> Result<()> {
    if *kind == SetKind::String {
        validate_string(name, value)?;
    }
    Ok(())
}

fn validate_string(name: &str, value: &str) -> Result<()> {
    if !value.bytes().all(|byte| (0x2d..=0x7e).contains(&byte)) {
        bail!("string attribute {name} must use only ASCII characters from '-' through '~'");
    }
    Ok(())
}

fn validate_token(kind: &str, value: &str) -> Result<()> {
    if value.is_empty() || value.contains(['\r', '\n']) {
        bail!("{kind} must be a nonempty single-line token");
    }
    Ok(())
}
