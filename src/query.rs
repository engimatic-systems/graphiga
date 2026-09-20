use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};
use nemo::api::{load_program_handle, reason};
use nemo::datavalues::DataValue;
use nemo::execution::{DefaultExecutionEngine, execution_parameters::ExecutionParameters};
use nemo::rule_model::components::{fact::Fact, tag::Tag, term::Term};
use nemo::rule_model::programs::{ProgramRead, ProgramWrite, handle::ProgramHandle};

use crate::document::Document;
use crate::schema::{AttributeKind, Direction, Schema, SetKind, Value};
use crate::store::NodeId;

const RESULT_PREDICATE: &str = "result";
const BASE_PREDICATES: [(&str, usize); 3] = [("node", 3), ("attr", 3), ("edge", 3)];

/// Executes one schema query against facts derived from the current documents.
///
/// Parsing, validation, planning, recursion, and stratified evaluation are owned
/// by Nemo. This adapter only checks Graphiga's base/result relation contract,
/// projects the document graph as native Nemo facts, and converts the unary
/// result relation back to existing node IDs.
pub(crate) async fn execute(
    schema: &Schema,
    documents: &BTreeMap<NodeId, Document>,
    source: &str,
) -> Result<BTreeSet<NodeId>> {
    let handle = load_program_handle(source.to_owned(), "schema query".to_owned())
        .map_err(|source| anyhow::anyhow!("invalid Nemo query: {source}"))?;
    validate_query_contract(&handle)?;

    let mut commit = handle.fork_full();
    for fact in project_facts(schema, documents)? {
        commit.add_fact(fact);
    }
    let handle = commit
        .submit()
        .map_err(|source| anyhow::anyhow!("add graph facts to Nemo query: {source:?}"))?;
    let mut engine = DefaultExecutionEngine::from_handle(handle, ExecutionParameters::default())
        .await
        .map_err(|source| anyhow::anyhow!("prepare Nemo query: {source}"))?;
    reason(&mut engine)
        .await
        .map_err(|source| anyhow::anyhow!("evaluate Nemo query: {source}"))?;

    let result = Tag::from(RESULT_PREDICATE);
    let Some(rows) = engine
        .predicate_rows(&result)
        .await
        .map_err(|source| anyhow::anyhow!("extract Nemo result relation: {source}"))?
    else {
        return Ok(BTreeSet::new());
    };
    let rows = rows.collect::<Vec<_>>();

    let mut ids = BTreeSet::new();
    for row in rows {
        let [value] = row.as_slice() else {
            bail!("query result relation must be unary");
        };
        let value = value.to_plain_string().ok_or_else(|| {
            anyhow::anyhow!("query result values must be node ID strings, got {value}")
        })?;
        let id = NodeId::parse(&value).context("interpret query result as a node ID")?;
        if !documents.contains_key(&id) {
            bail!("query returned unknown node ID {id}");
        }
        ids.insert(id);
    }

    Ok(ids)
}

fn validate_query_contract(program: &ProgramHandle) -> Result<()> {
    if program.imports().next().is_some() {
        bail!("schema queries may not contain Nemo import directives");
    }
    if program.exports().next().is_some() {
        bail!("schema queries may not contain Nemo export directives");
    }

    for fact in program.facts() {
        reject_base_definition(fact.predicate().name())?;
    }
    for rule in program.rules() {
        for atom in rule.head() {
            reject_base_definition(atom.predicate().name())?;
        }
    }

    let arities = program.arities();
    for (predicate, expected) in BASE_PREDICATES {
        if let Some(actual) = arities.get(&Tag::from(predicate))
            && *actual != expected
        {
            bail!("base relation {predicate} must have arity {expected}, found {actual}");
        }
    }

    let result = Tag::from(RESULT_PREDICATE);
    if !program.derived_predicates().contains(&result) {
        bail!("query must define {RESULT_PREDICATE}/1");
    }
    match arities.get(&result) {
        Some(1) => Ok(()),
        Some(arity) => bail!("query result relation must be unary, found arity {arity}"),
        None => bail!("query must define {RESULT_PREDICATE}/1"),
    }
}

fn reject_base_definition(predicate: &str) -> Result<()> {
    if BASE_PREDICATES
        .iter()
        .any(|(base, _arity)| *base == predicate)
    {
        bail!("query may not redefine read-only base relation {predicate}");
    }
    Ok(())
}

fn project_facts(schema: &Schema, documents: &BTreeMap<NodeId, Document>) -> Result<Vec<Fact>> {
    let mut facts = Vec::new();

    for (id, document) in documents {
        facts.push(fact(
            "node",
            [
                Term::from(id.as_str()),
                Term::from(document.node_type.clone()),
                Term::from(document.title()?),
            ],
        ));

        for (name, value) in &document.attributes {
            let kind = &schema.attribute(name)?.kind;
            match (kind, value) {
                (AttributeKind::Reference(direction), Value::Reference(target)) => {
                    push_edge(&mut facts, id.as_str(), name, target, *direction);
                }
                (AttributeKind::Set(SetKind::Reference(direction)), Value::Set(targets)) => {
                    for target in targets {
                        push_edge(&mut facts, id.as_str(), name, target, *direction);
                    }
                }
                (AttributeKind::Set(SetKind::String), Value::Set(values)) => {
                    for value in values {
                        facts.push(fact(
                            "attr",
                            [
                                Term::from(id.as_str()),
                                Term::from(name.as_str()),
                                Term::from(value.as_str()),
                            ],
                        ));
                    }
                }
                (_, value) => facts.push(fact(
                    "attr",
                    [
                        Term::from(id.as_str()),
                        Term::from(name.as_str()),
                        scalar_term(value),
                    ],
                )),
            }
        }
    }

    Ok(facts)
}

fn push_edge(
    facts: &mut Vec<Fact>,
    source: &str,
    attribute: &str,
    target: &str,
    direction: Direction,
) {
    facts.push(edge_fact(source, attribute, target));
    if direction == Direction::Undirected {
        facts.push(edge_fact(target, attribute, source));
    }
}

fn edge_fact(source: &str, attribute: &str, target: &str) -> Fact {
    fact(
        "edge",
        [
            Term::from(source),
            Term::from(attribute),
            Term::from(target),
        ],
    )
}

fn fact<const ARITY: usize>(predicate: &str, terms: [Term; ARITY]) -> Fact {
    Fact::new(Tag::from(predicate), terms)
}

fn scalar_term(value: &Value) -> Term {
    match value {
        Value::String(value) | Value::Enum(value) => Term::from(value.as_str()),
        Value::Boolean(value) => Term::from(*value),
        Value::Integer(value) => Term::from(*value),
        Value::Reference(_) | Value::Set(_) => {
            unreachable!("reference and set values are projected separately")
        }
    }
}
