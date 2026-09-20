use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};
use markdown::mdast::Node;
use markdown::{ParseOptions, to_mdast};

use crate::schema::{Schema, Value};

const FRONTMATTER_DELIMITER: &str = "+++";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Document {
    pub(crate) node_type: String,
    pub(crate) attributes: BTreeMap<String, Value>,
    body: String,
    stored_text: Option<String>,
}

#[derive(Clone, Debug)]
struct Heading {
    depth: u8,
    start: usize,
    end: usize,
    text: String,
}

impl Document {
    pub(crate) fn parse(text: &str, schema: &Schema) -> Result<Self> {
        let prefix = format!("{FRONTMATTER_DELIMITER}\n");
        let Some(rest) = text.strip_prefix(&prefix) else {
            bail!("document must begin with a standalone +++ line using LF newlines");
        };
        let marker = format!("\n{FRONTMATTER_DELIMITER}\n");
        let Some(closing) = rest.find(&marker) else {
            bail!("document has no closing standalone +++ line");
        };
        let frontmatter = &rest[..closing];
        let body = &rest[closing + marker.len()..];
        let mut table: toml::Table =
            toml::from_str(frontmatter).context("parse TOML frontmatter")?;
        let node_type = match table.remove("type") {
            Some(toml::Value::String(value)) => value,
            Some(_) => bail!("frontmatter type must be a string"),
            None => bail!("frontmatter is missing structural field type"),
        };
        schema.node_type(&node_type)?;

        let mut attributes = BTreeMap::new();
        for (name, raw_value) in table {
            let value = schema.value_from_toml(&name, &raw_value)?;
            attributes.insert(name, value);
        }
        for (name, attribute) in &schema.attributes {
            if attribute.required && !attributes.contains_key(name) {
                bail!("document is missing required attribute {name}");
            }
        }

        let document = Self {
            node_type,
            attributes,
            body: body.to_owned(),
            stored_text: Some(text.to_owned()),
        };
        document.title()?;
        Ok(document)
    }

    pub(crate) fn create(
        node_type: String,
        title: &str,
        sections: &[String],
        section_content: &BTreeMap<String, String>,
        attributes: BTreeMap<String, Value>,
    ) -> Result<Self> {
        let title = title.replace("\r\n", " ").replace(['\r', '\n'], " ");
        if title.is_empty() {
            bail!("title must not be empty");
        }

        let mut body = format!("# {title}\n");
        for section in sections {
            body.push_str("\n## ");
            body.push_str(section);
            body.push('\n');
            if let Some(content) = section_content.get(section) {
                body.push('\n');
                body.push_str(content);
                if !content.ends_with('\n') {
                    body.push('\n');
                }
            }
        }

        let document = Self {
            node_type,
            attributes,
            body,
            stored_text: None,
        };
        let parsed_title = document.title()?;
        if parsed_title != title {
            bail!("title is not representable as one plain Markdown H1");
        }
        Ok(document)
    }

    pub(crate) fn render(&self) -> Result<String> {
        let mut table = toml::Table::new();
        table.insert(
            "type".to_owned(),
            toml::Value::String(self.node_type.clone()),
        );
        for (name, value) in &self.attributes {
            table.insert(name.clone(), value.to_toml());
        }
        let frontmatter = toml::to_string(&table).context("serialize TOML frontmatter")?;
        Ok(format!(
            "{FRONTMATTER_DELIMITER}\n{frontmatter}{FRONTMATTER_DELIMITER}\n{}",
            self.body
        ))
    }

    pub(crate) fn full_text(&self) -> Result<String> {
        match &self.stored_text {
            Some(text) => Ok(text.clone()),
            None => self.render(),
        }
    }

    pub(crate) fn title(&self) -> Result<String> {
        let headings = headings(&self.body)?;
        let titles: Vec<_> = headings
            .iter()
            .filter(|heading| heading.depth == 1)
            .collect();
        match titles.as_slice() {
            [title] if !title.text.is_empty() => Ok(title.text.clone()),
            [] => bail!("Markdown body has no H1 title"),
            [_] => bail!("Markdown body has an empty H1 title"),
            _ => bail!("Markdown body has more than one H1 title"),
        }
    }

    pub(crate) fn outline(&self, section: Option<&str>) -> Result<String> {
        let headings = headings(&self.body)?;
        let selected = select_headings(&headings, section)?;
        let mut output = selected
            .iter()
            .map(|heading| self.body[heading.start..heading.end].to_owned())
            .collect::<Vec<_>>()
            .join("\n");
        if !output.is_empty() {
            output.push('\n');
        }
        Ok(output)
    }

    pub(crate) fn section_text(&self, name: &str) -> Result<String> {
        let headings = headings(&self.body)?;
        let start_index = headings
            .iter()
            .position(|heading| heading.depth == 2 && heading.text == name)
            .ok_or_else(|| anyhow::anyhow!("section not found: {name}"))?;
        let start = headings[start_index].start;
        let end = headings[start_index + 1..]
            .iter()
            .find(|heading| heading.depth <= 2)
            .map_or(self.body.len(), |heading| heading.start);
        Ok(self.body[start..end].to_owned())
    }

    pub(crate) fn reference_targets(&self, attribute: &str) -> BTreeSet<String> {
        match self.attributes.get(attribute) {
            Some(Value::Reference(target)) => [target.clone()].into_iter().collect(),
            Some(Value::Set(targets)) => targets.clone(),
            _ => BTreeSet::new(),
        }
    }
}

fn headings(body: &str) -> Result<Vec<Heading>> {
    let tree =
        to_mdast(body, &ParseOptions::default()).map_err(|message| anyhow::anyhow!(message))?;
    let mut result = Vec::new();
    collect_headings(&tree, &mut result);
    result.sort_by_key(|heading| heading.start);
    Ok(result)
}

fn collect_headings(node: &Node, output: &mut Vec<Heading>) {
    if let Node::Heading(heading) = node
        && let Some(position) = &heading.position
    {
        let mut text = String::new();
        collect_text(node, &mut text);
        output.push(Heading {
            depth: heading.depth,
            start: position.start.offset,
            end: position.end.offset,
            text,
        });
    }
    if let Some(children) = node.children() {
        for child in children {
            collect_headings(child, output);
        }
    }
}

fn collect_text(node: &Node, output: &mut String) {
    if let Node::Text(text) = node {
        output.push_str(&text.value);
    }
    if let Some(children) = node.children() {
        for child in children {
            collect_text(child, output);
        }
    }
}

fn select_headings<'a>(headings: &'a [Heading], section: Option<&str>) -> Result<Vec<&'a Heading>> {
    let Some(name) = section else {
        return Ok(headings.iter().collect());
    };
    let start = headings
        .iter()
        .position(|heading| heading.depth == 2 && heading.text == name)
        .ok_or_else(|| anyhow::anyhow!("section not found: {name}"))?;
    let mut selected = vec![&headings[start]];
    for heading in &headings[start + 1..] {
        if heading.depth <= 2 {
            break;
        }
        selected.push(heading);
    }
    Ok(selected)
}
