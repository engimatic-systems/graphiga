mod document;
mod graph;
mod query;
mod schema;
mod store;

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{ArgAction, Parser, Subcommand};
use graph::{Graph, NodeRecord};
use store::{NodeId, Store};

#[derive(Parser)]
#[command(version, about = "A schema-directed Markdown document graph")]
struct Cli {
    /// Flat store containing schema.toml and node Markdown files.
    #[arg(long)]
    store: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a node without overwriting an existing node.
    Create {
        node_type: String,
        id: String,
        #[arg(long, allow_hyphen_values = true)]
        title: String,
        /// Set an initial attribute. Repeat as: --set ATTRIBUTE VALUE.
        #[arg(long = "set", value_names = ["ATTRIBUTE", "VALUE"], num_args = 2, action = ArgAction::Append, allow_hyphen_values = true)]
        sets: Vec<String>,
        /// Supply initial section content. Repeat as: --section NAME TEXT.
        #[arg(long = "section", value_names = ["NAME", "TEXT"], num_args = 2, action = ArgAction::Append, allow_hyphen_values = true)]
        sections: Vec<String>,
    },
    /// List IDs, node types, and H1 titles in ID order.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Execute a named schema-resident query.
    Query {
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// Show a node outline or complete stored text.
    Show {
        id: String,
        #[arg(long)]
        full: bool,
        #[arg(long)]
        section: Option<String>,
    },
    /// Inspect an outgoing neighborhood or directed incoming neighborhood.
    Edges {
        id: String,
        attribute: String,
        #[arg(long)]
        incoming: bool,
        #[arg(long)]
        json: bool,
    },
    /// Assign a scalar or complete comma-separated set; omit VALUE for an empty set.
    Set {
        id: String,
        attribute: String,
        #[arg(allow_hyphen_values = true)]
        value: Option<String>,
    },
    /// Remove an optional attribute.
    Unset { id: String, attribute: String },
    /// Add comma-separated members to a set.
    Add {
        id: String,
        attribute: String,
        #[arg(allow_hyphen_values = true)]
        members: String,
    },
    /// Remove comma-separated members from a set.
    Remove {
        id: String,
        attribute: String,
        #[arg(allow_hyphen_values = true)]
        members: String,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let store = Store::load(&cli.store)?;
    let graph = Graph::new(&store.schema, &store.documents);

    match cli.command {
        Command::Create {
            node_type,
            id,
            title,
            sets,
            sections,
        } => {
            let id = NodeId::parse(&id)?;
            let sets = grouped_values("--set", sets)?;
            let sections = grouped_values("--section", sections)?;
            let changes = graph.create(&node_type, &id, &title, &sets, &sections)?;
            store.persist(&changes)?;
        }
        Command::List { json } => print_records(&graph.list()?, json)?,
        Command::Query { name, json } => print_records(&graph.query(&name).await?, json)?,
        Command::Show { id, full, section } => {
            let id = NodeId::parse(&id)?;
            let document = graph.show(&id)?;
            let output = match (full, section.as_deref()) {
                (true, None) => document.full_text()?,
                (true, Some(section)) => document.section_text(section)?,
                (false, section) => document.outline(section)?,
            };
            print_exact(&output)?;
        }
        Command::Edges {
            id,
            attribute,
            incoming,
            json,
        } => {
            let id = NodeId::parse(&id)?;
            print_records(&graph.edges(&id, &attribute, incoming)?, json)?;
        }
        Command::Set {
            id,
            attribute,
            value,
        } => {
            let id = NodeId::parse(&id)?;
            let changes = graph.set(&id, &attribute, value.as_deref())?;
            store.persist(&changes)?;
        }
        Command::Unset { id, attribute } => {
            let id = NodeId::parse(&id)?;
            let changes = graph.unset(&id, &attribute)?;
            store.persist(&changes)?;
        }
        Command::Add {
            id,
            attribute,
            members,
        } => {
            let id = NodeId::parse(&id)?;
            let changes = graph.add(&id, &attribute, &members)?;
            store.persist(&changes)?;
        }
        Command::Remove {
            id,
            attribute,
            members,
        } => {
            let id = NodeId::parse(&id)?;
            let changes = graph.remove(&id, &attribute, &members)?;
            store.persist(&changes)?;
        }
    }
    Ok(())
}

fn grouped_values(option: &str, values: Vec<String>) -> Result<BTreeMap<String, String>> {
    let (pairs, remainder) = values.as_chunks::<2>();
    if !remainder.is_empty() {
        bail!("{option} requires exactly two values per occurrence");
    }
    let mut groups = BTreeMap::new();
    for pair in pairs {
        if groups.insert(pair[0].clone(), pair[1].clone()).is_some() {
            bail!("{option} repeats name {}", pair[0]);
        }
    }
    Ok(groups)
}

fn print_records(records: &[NodeRecord], json: bool) -> Result<()> {
    if json {
        let output = serde_json::to_string_pretty(records).context("serialize JSON output")?;
        println!("{output}");
    } else {
        for record in records {
            println!("{}  {}  {}", record.id, record.node_type, record.title);
        }
    }
    Ok(())
}

fn print_exact(output: &str) -> Result<()> {
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(output.as_bytes())
        .context("write stdout")?;
    stdout.flush().context("flush stdout")
}
