use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};
use datafrog::{Iteration, PrefixFilter, Relation, Variable};

use crate::document::Document;
use crate::schema::{AttributeKind, Schema, SetKind, Value};
use crate::store::NodeId;

const NODE_RELATION: &str = "node";
const ATTR_RELATION: &str = "attr";
const EDGE_RELATION: &str = "edge";
const RESULT_RELATION: &str = "result";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Scalar {
    String(String),
    Boolean(bool),
    Integer(i64),
}

type Row = Vec<Scalar>;
type Binding = Vec<Option<Scalar>>;

#[derive(Clone, Debug)]
enum NamedTerm {
    Variable(String),
    Constant(Scalar),
}

#[derive(Clone, Debug)]
struct NamedAtom {
    relation: String,
    terms: Vec<NamedTerm>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Comparison {
    Equal,
    NotEqual,
}

#[derive(Clone, Debug)]
enum NamedLiteral {
    Positive(NamedAtom),
    Negative(NamedAtom),
    Filter(NamedTerm, Comparison, NamedTerm),
}

#[derive(Clone, Debug)]
struct NamedRule {
    head: NamedAtom,
    body: Vec<NamedLiteral>,
}

#[derive(Clone, Debug)]
enum Term {
    Variable(usize),
    Constant(Scalar),
}

#[derive(Clone, Debug)]
struct Atom {
    relation: String,
    terms: Vec<Term>,
}

#[derive(Clone, Debug)]
struct Filter {
    left: Term,
    comparison: Comparison,
    right: Term,
}

#[derive(Clone, Debug)]
struct Rule {
    head: Atom,
    positive: Vec<Atom>,
    negative: Vec<Atom>,
    filters: Vec<Filter>,
    variable_count: usize,
}

#[derive(Debug)]
struct Program {
    rules: Vec<Rule>,
    strata: BTreeMap<String, usize>,
}

/// Parses, validates, plans, and evaluates one query-local Datalog program.
///
/// The document graph remains authoritative: all input facts are projected for
/// this call, and only the unary `result` relation is returned.
pub(crate) fn evaluate(
    source: &str,
    schema: &Schema,
    documents: &BTreeMap<NodeId, Document>,
) -> Result<BTreeSet<NodeId>> {
    let named = Parser::parse(source)?;
    let program = Program::bind(named)?;
    let facts = project_facts(schema, documents)?;
    let derived = program.evaluate(facts)?;
    let mut result = BTreeSet::new();
    for row in derived
        .get(RESULT_RELATION)
        .into_iter()
        .flat_map(|relation| relation.iter())
    {
        let [Scalar::String(id)] = row.as_slice() else {
            bail!("result must contain one string node ID");
        };
        let id = NodeId::parse(id)?;
        if !documents.contains_key(&id) {
            bail!("query result names unknown node {id}");
        }
        result.insert(id);
    }
    Ok(result)
}

impl Program {
    fn bind(named_rules: Vec<NamedRule>) -> Result<Self> {
        if named_rules.is_empty() {
            bail!("query program must contain at least one rule");
        }

        let base_arities = BTreeMap::from([
            (NODE_RELATION.to_owned(), 3),
            (ATTR_RELATION.to_owned(), 3),
            (EDGE_RELATION.to_owned(), 3),
        ]);
        let mut arities = base_arities.clone();
        let mut defined = BTreeSet::new();
        for rule in &named_rules {
            if base_arities.contains_key(&rule.head.relation) {
                bail!("base relation {} is read-only", rule.head.relation);
            }
            declare_arity(&mut arities, &rule.head)?;
            defined.insert(rule.head.relation.clone());
        }
        for rule in &named_rules {
            for literal in &rule.body {
                match literal {
                    NamedLiteral::Positive(atom) | NamedLiteral::Negative(atom) => {
                        declare_arity(&mut arities, atom)?;
                        if !base_arities.contains_key(&atom.relation)
                            && !defined.contains(&atom.relation)
                        {
                            bail!("relation {} is not defined", atom.relation);
                        }
                    }
                    NamedLiteral::Filter(_, _, _) => {}
                }
            }
        }
        match arities.get(RESULT_RELATION) {
            Some(1) => {}
            Some(arity) => bail!("result relation must be unary, not arity {arity}"),
            None => bail!("query program must define unary result(NodeId)"),
        }

        let mut rules = Vec::new();
        for named in named_rules {
            let mut variables = BTreeMap::new();
            for atom in named.body.iter().filter_map(|literal| match literal {
                NamedLiteral::Positive(atom) => Some(atom),
                _ => None,
            }) {
                for term in &atom.terms {
                    if let NamedTerm::Variable(name) = term {
                        let next = variables.len();
                        variables.entry(name.clone()).or_insert(next);
                    }
                }
            }
            let head = bind_atom(&named.head, &variables, "head")?;
            let mut positive = Vec::new();
            let mut negative = Vec::new();
            let mut filters = Vec::new();
            for literal in &named.body {
                match literal {
                    NamedLiteral::Positive(atom) => {
                        positive.push(bind_atom(atom, &variables, "positive atom")?)
                    }
                    NamedLiteral::Negative(atom) => {
                        negative.push(bind_atom(atom, &variables, "negated atom")?)
                    }
                    NamedLiteral::Filter(left, comparison, right) => filters.push(Filter {
                        left: bind_term(left, &variables, "filter")?,
                        comparison: *comparison,
                        right: bind_term(right, &variables, "filter")?,
                    }),
                }
            }
            rules.push(Rule {
                head,
                positive,
                negative,
                filters,
                variable_count: variables.len(),
            });
        }

        let strata = stratify(&rules, &defined)?;
        Ok(Self { rules, strata })
    }

    fn evaluate(
        &self,
        mut facts: BTreeMap<String, Relation<Row>>,
    ) -> Result<BTreeMap<String, Relation<Row>>> {
        let max_stratum = self.strata.values().copied().max().unwrap_or(0);
        for stratum in 0..=max_stratum {
            let rules: Vec<_> = self
                .rules
                .iter()
                .filter(|rule| self.strata[&rule.head.relation] == stratum)
                .collect();
            if rules.is_empty() {
                continue;
            }
            let heads: BTreeSet<_> = rules
                .iter()
                .map(|rule| rule.head.relation.clone())
                .collect();
            let mut relation_names = heads.clone();
            for rule in &rules {
                relation_names.extend(rule.positive.iter().map(|atom| atom.relation.clone()));
            }

            let mut iteration = Iteration::new();
            let mut variables = BTreeMap::new();
            for name in relation_names {
                let variable = iteration.variable::<Row>(&name);
                if let Some(fixed) = facts.get(&name) {
                    variable.insert(fixed.clone());
                }
                variables.insert(name, variable);
            }
            let plans = rules
                .into_iter()
                .enumerate()
                .map(|(index, rule)| RulePlan::new(index, rule, &facts, &variables, &mut iteration))
                .collect::<Result<Vec<_>>>()?;

            while iteration.changed() {
                for plan in &plans {
                    plan.apply();
                }
            }
            drop(plans);
            drop(iteration);
            for head in heads {
                let relation = variables
                    .remove(&head)
                    .expect("head relation has an evaluation variable")
                    .complete();
                facts.insert(head, relation);
            }
        }
        Ok(facts)
    }
}

fn declare_arity(arities: &mut BTreeMap<String, usize>, atom: &NamedAtom) -> Result<()> {
    match arities.get(&atom.relation) {
        Some(arity) if *arity != atom.terms.len() => bail!(
            "relation {} is used with arities {arity} and {}",
            atom.relation,
            atom.terms.len()
        ),
        _ => {
            arities.insert(atom.relation.clone(), atom.terms.len());
            Ok(())
        }
    }
}

fn bind_atom(
    atom: &NamedAtom,
    variables: &BTreeMap<String, usize>,
    position: &str,
) -> Result<Atom> {
    Ok(Atom {
        relation: atom.relation.clone(),
        terms: atom
            .terms
            .iter()
            .map(|term| bind_term(term, variables, position))
            .collect::<Result<_>>()?,
    })
}

fn bind_term(
    term: &NamedTerm,
    variables: &BTreeMap<String, usize>,
    position: &str,
) -> Result<Term> {
    match term {
        NamedTerm::Constant(value) => Ok(Term::Constant(value.clone())),
        NamedTerm::Variable(name) => {
            variables
                .get(name)
                .copied()
                .map(Term::Variable)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "unsafe {position} variable {name} is not bound by a positive atom"
                    )
                })
        }
    }
}

fn stratify(rules: &[Rule], defined: &BTreeSet<String>) -> Result<BTreeMap<String, usize>> {
    let mut strata: BTreeMap<_, _> = defined.iter().map(|name| (name.clone(), 0)).collect();
    // Include the three base relations in the relaxation bound. A negative
    // dependency on a base relation legitimately raises even a one-relation
    // program; only continued growth past all relation vertices proves a cycle.
    let relation_count = strata.len() + 3;
    for round in 0..relation_count {
        let mut changed = false;
        for rule in rules {
            let required = rule
                .positive
                .iter()
                .map(|atom| strata.get(&atom.relation).copied().unwrap_or(0))
                .chain(
                    rule.negative
                        .iter()
                        .map(|atom| strata.get(&atom.relation).copied().unwrap_or(0) + 1),
                )
                .max()
                .unwrap_or(0);
            let head = strata
                .get_mut(&rule.head.relation)
                .expect("all head relations were declared");
            if *head < required {
                *head = required;
                changed = true;
            }
        }
        if !changed {
            return Ok(strata);
        }
        if round + 1 == relation_count {
            bail!("program has recursive negation and cannot be stratified");
        }
    }
    Ok(strata)
}

struct RulePlan {
    first: Option<FirstStep>,
    joins: Vec<JoinStep>,
    negatives: Vec<NegativeStep>,
    filters: Vec<FilterStep>,
    head: HeadStep,
}

impl RulePlan {
    fn new(
        plan_index: usize,
        rule: &Rule,
        fixed: &BTreeMap<String, Relation<Row>>,
        relations: &BTreeMap<String, Variable<Row>>,
        iteration: &mut Iteration,
    ) -> Result<Self> {
        let first_output = iteration.variable::<Binding>(&format!("rule-{plan_index}-first"));
        let (first, mut bound) = if let Some(first_atom) = rule.positive.first() {
            (
                Some(FirstStep {
                    relation: relations[&first_atom.relation].clone(),
                    output: first_output.clone(),
                    atom: first_atom.clone(),
                    variable_count: rule.variable_count,
                }),
                first_atom.variable_indexes(),
            )
        } else {
            first_output.insert(Relation::from_vec(vec![vec![None; rule.variable_count]]));
            (None, BTreeSet::new())
        };
        let mut current = first_output;
        let mut joins = Vec::new();
        for (step_index, atom) in rule.positive.iter().skip(1).enumerate() {
            let left = iteration.variable::<(JoinKey, Binding)>(&format!(
                "rule-{plan_index}-join-{step_index}-left"
            ));
            let right = iteration
                .variable::<(JoinKey, Row)>(&format!("rule-{plan_index}-join-{step_index}-right"));
            let output = iteration
                .variable::<Binding>(&format!("rule-{plan_index}-join-{step_index}-output"));
            joins.push(JoinStep {
                input: current,
                relation: relations[&atom.relation].clone(),
                left,
                right,
                output: output.clone(),
                atom: atom.clone(),
                previously_bound: bound.clone(),
            });
            bound.extend(atom.variable_indexes());
            current = output;
        }

        let mut negatives = Vec::new();
        for (step_index, atom) in rule.negative.iter().enumerate() {
            let keyed = iteration.variable::<(Row, Binding)>(&format!(
                "rule-{plan_index}-negative-{step_index}-keyed"
            ));
            let output = iteration
                .variable::<Binding>(&format!("rule-{plan_index}-negative-{step_index}-output"));
            let negative = fixed
                .get(&atom.relation)
                .cloned()
                .unwrap_or_else(|| Relation::from_vec(Vec::new()));
            negatives.push(NegativeStep {
                input: current,
                keyed,
                negative,
                output: output.clone(),
                atom: atom.clone(),
            });
            current = output;
        }

        let mut filters = Vec::new();
        for (step_index, filter) in rule.filters.iter().enumerate() {
            let output =
                iteration.variable::<Binding>(&format!("rule-{plan_index}-filter-{step_index}"));
            filters.push(FilterStep {
                input: current,
                output: output.clone(),
                filter: filter.clone(),
            });
            current = output;
        }
        let head = HeadStep {
            input: current,
            output: relations[&rule.head.relation].clone(),
            terms: rule.head.terms.clone(),
        };
        Ok(Self {
            first,
            joins,
            negatives,
            filters,
            head,
        })
    }

    fn apply(&self) {
        if let Some(first) = &self.first {
            first.apply();
        }
        for join in &self.joins {
            join.apply();
        }
        for negative in &self.negatives {
            negative.apply();
        }
        for filter in &self.filters {
            filter.apply();
        }
        self.head.apply();
    }
}

struct FirstStep {
    relation: Variable<Row>,
    output: Variable<Binding>,
    atom: Atom,
    variable_count: usize,
}

impl FirstStep {
    fn apply(&self) {
        let atom = &self.atom;
        let variable_count = self.variable_count;
        self.output.from_leapjoin(
            &self.relation,
            PrefixFilter::from(|row: &Row| atom.matches_row(row)),
            |row, &()| atom.extend_binding(&vec![None; variable_count], row),
        );
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct JoinKey {
    valid: bool,
    values: Vec<Scalar>,
}

struct JoinStep {
    input: Variable<Binding>,
    relation: Variable<Row>,
    left: Variable<(JoinKey, Binding)>,
    right: Variable<(JoinKey, Row)>,
    output: Variable<Binding>,
    atom: Atom,
    previously_bound: BTreeSet<usize>,
}

impl JoinStep {
    fn apply(&self) {
        let atom = &self.atom;
        let bound = &self.previously_bound;
        self.left.from_map(&self.input, |binding| {
            (atom.binding_key(binding, bound), binding.clone())
        });
        self.right.from_map(&self.relation, |row| {
            (atom.row_key(row, bound), row.clone())
        });
        self.output
            .from_join(&self.left, &self.right, |_key, binding, row| {
                atom.extend_binding(binding, row)
            });
    }
}

struct NegativeStep {
    input: Variable<Binding>,
    keyed: Variable<(Row, Binding)>,
    negative: Relation<Row>,
    output: Variable<Binding>,
    atom: Atom,
}

impl NegativeStep {
    fn apply(&self) {
        let atom = &self.atom;
        self.keyed.from_map(&self.input, |binding| {
            (atom.instantiate(binding), binding.clone())
        });
        self.output
            .from_antijoin(&self.keyed, &self.negative, |_row, binding| binding.clone());
    }
}

struct FilterStep {
    input: Variable<Binding>,
    output: Variable<Binding>,
    filter: Filter,
}

impl FilterStep {
    fn apply(&self) {
        let filter = &self.filter;
        self.output.from_leapjoin(
            &self.input,
            PrefixFilter::from(|binding: &Binding| filter.matches(binding)),
            |binding, &()| binding.clone(),
        );
    }
}

struct HeadStep {
    input: Variable<Binding>,
    output: Variable<Row>,
    terms: Vec<Term>,
}

impl HeadStep {
    fn apply(&self) {
        let terms = &self.terms;
        self.output.from_map(&self.input, |binding| {
            terms
                .iter()
                .map(|term| term.value(binding).clone())
                .collect()
        });
    }
}

impl Atom {
    fn variable_indexes(&self) -> BTreeSet<usize> {
        self.terms
            .iter()
            .filter_map(|term| match term {
                Term::Variable(index) => Some(*index),
                Term::Constant(_) => None,
            })
            .collect()
    }

    fn matches_row(&self, row: &Row) -> bool {
        if row.len() != self.terms.len() {
            return false;
        }
        let mut seen = BTreeMap::new();
        self.terms.iter().zip(row).all(|(term, value)| match term {
            Term::Constant(constant) => constant == value,
            Term::Variable(index) => match seen.get(index) {
                Some(previous) => *previous == value,
                None => {
                    seen.insert(*index, value);
                    true
                }
            },
        })
    }

    fn extend_binding(&self, binding: &Binding, row: &Row) -> Binding {
        let mut result = binding.clone();
        for (term, value) in self.terms.iter().zip(row) {
            if let Term::Variable(index) = term {
                match &result[*index] {
                    Some(previous) => debug_assert_eq!(previous, value),
                    None => result[*index] = Some(value.clone()),
                }
            }
        }
        result
    }

    fn binding_key(&self, binding: &Binding, bound: &BTreeSet<usize>) -> JoinKey {
        JoinKey {
            valid: true,
            values: self
                .terms
                .iter()
                .filter_map(|term| match term {
                    Term::Constant(value) => Some(value.clone()),
                    Term::Variable(index) if bound.contains(index) => Some(
                        binding[*index]
                            .as_ref()
                            .expect("bound variable has a value")
                            .clone(),
                    ),
                    Term::Variable(_) => None,
                })
                .collect(),
        }
    }

    fn row_key(&self, row: &Row, bound: &BTreeSet<usize>) -> JoinKey {
        let mut seen = BTreeMap::new();
        let mut valid = row.len() == self.terms.len();
        let mut values = Vec::new();
        for (term, value) in self.terms.iter().zip(row) {
            match term {
                Term::Constant(constant) => {
                    valid &= constant == value;
                    values.push(constant.clone());
                }
                Term::Variable(index) => {
                    if let Some(previous) = seen.insert(*index, value) {
                        valid &= previous == value;
                    }
                    if bound.contains(index) {
                        values.push(value.clone());
                    }
                }
            }
        }
        JoinKey { valid, values }
    }

    fn instantiate(&self, binding: &Binding) -> Row {
        self.terms
            .iter()
            .map(|term| term.value(binding).clone())
            .collect()
    }
}

impl Term {
    fn value<'a>(&'a self, binding: &'a Binding) -> &'a Scalar {
        match self {
            Self::Constant(value) => value,
            Self::Variable(index) => binding[*index]
                .as_ref()
                .expect("safety and positive evaluation bind every variable"),
        }
    }
}

impl Filter {
    fn matches(&self, binding: &Binding) -> bool {
        let equal = self.left.value(binding) == self.right.value(binding);
        match self.comparison {
            Comparison::Equal => equal,
            Comparison::NotEqual => !equal,
        }
    }
}

fn project_facts(
    schema: &Schema,
    documents: &BTreeMap<NodeId, Document>,
) -> Result<BTreeMap<String, Relation<Row>>> {
    let mut nodes = Vec::new();
    let mut attrs = Vec::new();
    let mut edges = Vec::new();
    for (id, document) in documents {
        nodes.push(vec![
            Scalar::String(id.to_string()),
            Scalar::String(document.node_type.clone()),
            Scalar::String(document.title()?),
        ]);
        for (name, value) in &document.attributes {
            let declaration = schema.attribute(name)?;
            match (&declaration.kind, value) {
                (AttributeKind::String | AttributeKind::Enum(_), Value::String(value))
                | (AttributeKind::Enum(_), Value::Enum(value)) => attrs.push(vec![
                    Scalar::String(id.to_string()),
                    Scalar::String(name.clone()),
                    Scalar::String(value.clone()),
                ]),
                (AttributeKind::Boolean, Value::Boolean(value)) => attrs.push(vec![
                    Scalar::String(id.to_string()),
                    Scalar::String(name.clone()),
                    Scalar::Boolean(*value),
                ]),
                (AttributeKind::Integer, Value::Integer(value)) => attrs.push(vec![
                    Scalar::String(id.to_string()),
                    Scalar::String(name.clone()),
                    Scalar::Integer(*value),
                ]),
                (AttributeKind::Set(SetKind::String), Value::Set(values)) => {
                    for value in values {
                        attrs.push(vec![
                            Scalar::String(id.to_string()),
                            Scalar::String(name.clone()),
                            Scalar::String(value.clone()),
                        ]);
                    }
                }
                (AttributeKind::Reference(_), Value::Reference(target)) => edges.push(vec![
                    Scalar::String(id.to_string()),
                    Scalar::String(name.clone()),
                    Scalar::String(target.clone()),
                ]),
                (AttributeKind::Set(SetKind::Reference(_)), Value::Set(targets)) => {
                    for target in targets {
                        edges.push(vec![
                            Scalar::String(id.to_string()),
                            Scalar::String(name.clone()),
                            Scalar::String(target.clone()),
                        ]);
                    }
                }
                _ => unreachable!("schema parsing fixes each attribute's value shape"),
            }
        }
    }
    Ok(BTreeMap::from([
        (NODE_RELATION.to_owned(), Relation::from_vec(nodes)),
        (ATTR_RELATION.to_owned(), Relation::from_vec(attrs)),
        (EDGE_RELATION.to_owned(), Relation::from_vec(edges)),
    ]))
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Identifier(String),
    String(String),
    Integer(i64),
    Boolean(bool),
    Not,
    LeftParen,
    RightParen,
    Comma,
    Dot,
    ColonDash,
    Equal,
    NotEqual,
}

#[derive(Clone, Debug)]
struct SpannedToken {
    token: Token,
    offset: usize,
}

struct Parser {
    tokens: Vec<SpannedToken>,
    cursor: usize,
}

impl Parser {
    fn parse(source: &str) -> Result<Vec<NamedRule>> {
        let mut parser = Self {
            tokens: lex(source)?,
            cursor: 0,
        };
        let mut rules = Vec::new();
        while parser.peek().is_some() {
            rules.push(parser.rule()?);
        }
        Ok(rules)
    }

    fn rule(&mut self) -> Result<NamedRule> {
        let head = self.atom()?;
        self.expect(Token::ColonDash)?;
        let mut body = Vec::new();
        loop {
            body.push(self.literal()?);
            if self.take(&Token::Comma) {
                continue;
            }
            self.expect(Token::Dot)?;
            break;
        }
        Ok(NamedRule { head, body })
    }

    fn literal(&mut self) -> Result<NamedLiteral> {
        if self.take(&Token::Not) {
            return Ok(NamedLiteral::Negative(self.atom()?));
        }
        if matches!(self.peek(), Some(Token::Identifier(name)) if starts_lowercase(name)) {
            return Ok(NamedLiteral::Positive(self.atom()?));
        }
        let left = self.term()?;
        let comparison = if self.take(&Token::Equal) {
            Comparison::Equal
        } else if self.take(&Token::NotEqual) {
            Comparison::NotEqual
        } else {
            return self.error("expected relation atom, '=' or '!='");
        };
        let right = self.term()?;
        Ok(NamedLiteral::Filter(left, comparison, right))
    }

    fn atom(&mut self) -> Result<NamedAtom> {
        let relation = match self.next() {
            Some(Token::Identifier(name)) if starts_lowercase(&name) => name,
            _ => return self.error("expected a lowercase relation name"),
        };
        self.expect(Token::LeftParen)?;
        let mut terms = vec![self.term()?];
        while self.take(&Token::Comma) {
            terms.push(self.term()?);
        }
        self.expect(Token::RightParen)?;
        Ok(NamedAtom { relation, terms })
    }

    fn term(&mut self) -> Result<NamedTerm> {
        match self.next() {
            Some(Token::Identifier(name)) if starts_uppercase(&name) => {
                Ok(NamedTerm::Variable(name))
            }
            Some(Token::String(value)) => Ok(NamedTerm::Constant(Scalar::String(value))),
            Some(Token::Integer(value)) => Ok(NamedTerm::Constant(Scalar::Integer(value))),
            Some(Token::Boolean(value)) => Ok(NamedTerm::Constant(Scalar::Boolean(value))),
            _ => self.error("expected an uppercase variable or scalar constant"),
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.cursor).map(|token| &token.token)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.cursor)?.token.clone();
        self.cursor += 1;
        Some(token)
    }

    fn take(&mut self, expected: &Token) -> bool {
        if self.peek() == Some(expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: Token) -> Result<()> {
        if self.take(&expected) {
            Ok(())
        } else {
            self.error(&format!("expected {expected:?}"))
        }
    }

    fn error<T>(&self, message: &str) -> Result<T> {
        let offset = self.tokens.get(self.cursor).map_or_else(
            || self.tokens.last().map_or(0, |token| token.offset + 1),
            |token| token.offset,
        );
        bail!("Datalog syntax error at byte {offset}: {message}")
    }
}

fn lex(source: &str) -> Result<Vec<SpannedToken>> {
    let bytes = source.as_bytes();
    let mut cursor = 0;
    let mut tokens = Vec::new();
    while cursor < bytes.len() {
        if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        let offset = cursor;
        let token = match bytes[cursor] {
            b'(' => {
                cursor += 1;
                Token::LeftParen
            }
            b')' => {
                cursor += 1;
                Token::RightParen
            }
            b',' => {
                cursor += 1;
                Token::Comma
            }
            b'.' => {
                cursor += 1;
                Token::Dot
            }
            b'=' => {
                cursor += 1;
                Token::Equal
            }
            b'!' if bytes.get(cursor + 1) == Some(&b'=') => {
                cursor += 2;
                Token::NotEqual
            }
            b':' if bytes.get(cursor + 1) == Some(&b'-') => {
                cursor += 2;
                Token::ColonDash
            }
            b'"' => {
                cursor += 1;
                let mut value = String::new();
                let mut closed = false;
                while cursor < bytes.len() {
                    match bytes[cursor] {
                        b'"' => {
                            cursor += 1;
                            closed = true;
                            break;
                        }
                        b'\\' => {
                            cursor += 1;
                            let escaped = match bytes.get(cursor) {
                                Some(b'"') => '"',
                                Some(b'\\') => '\\',
                                Some(b'n') => '\n',
                                Some(b'r') => '\r',
                                Some(b't') => '\t',
                                _ => bail!(
                                    "Datalog syntax error at byte {cursor}: invalid string escape"
                                ),
                            };
                            value.push(escaped);
                            cursor += 1;
                        }
                        byte if byte.is_ascii() && !byte.is_ascii_control() => {
                            value.push(byte as char);
                            cursor += 1;
                        }
                        _ => bail!(
                            "Datalog syntax error at byte {cursor}: strings must contain printable ASCII or supported escapes"
                        ),
                    }
                }
                if !closed {
                    bail!("Datalog syntax error at byte {offset}: unterminated string");
                }
                Token::String(value)
            }
            b'-' if bytes.get(cursor + 1).is_some_and(u8::is_ascii_digit) => {
                cursor += 1;
                while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
                    cursor += 1;
                }
                let value = source[offset..cursor].parse::<i64>().map_err(|_| {
                    anyhow::anyhow!(
                        "Datalog integer at byte {offset} is outside signed 64-bit range"
                    )
                })?;
                Token::Integer(value)
            }
            byte if byte.is_ascii_digit() => {
                cursor += 1;
                while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
                    cursor += 1;
                }
                let value = source[offset..cursor].parse::<i64>().map_err(|_| {
                    anyhow::anyhow!(
                        "Datalog integer at byte {offset} is outside signed 64-bit range"
                    )
                })?;
                Token::Integer(value)
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                cursor += 1;
                while bytes
                    .get(cursor)
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
                {
                    cursor += 1;
                }
                match &source[offset..cursor] {
                    "not" => Token::Not,
                    "true" => Token::Boolean(true),
                    "false" => Token::Boolean(false),
                    name => Token::Identifier(name.to_owned()),
                }
            }
            _ => bail!(
                "Datalog syntax error at byte {offset}: unexpected character {:?}",
                source[offset..]
                    .chars()
                    .next()
                    .expect("cursor is in bounds")
            ),
        };
        tokens.push(SpannedToken { token, offset });
    }
    Ok(tokens)
}

fn starts_lowercase(value: &str) -> bool {
    value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
}

fn starts_uppercase(value: &str) -> bool {
    value.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
}
