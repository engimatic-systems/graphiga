use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};

use crate::adjacency::ReferenceAttribute;
use crate::document::Document;
use crate::schema::{AttributeKind, Schema, SetKind, Value};
use crate::store::NodeId;

/// A schema-bound predicate.
///
/// Parsing resolves every field and literal type before documents are examined,
/// so malformed or type-incompatible declarations fail even for an empty store.
pub(crate) struct Predicate {
    expression: Expression,
}

impl Predicate {
    pub(crate) fn parse(input: &str, schema: &Schema) -> Result<Self> {
        let tokens = tokenize(input)?;
        let mut parser = Parser {
            tokens,
            position: 0,
        };
        let expression = parser.parse_expression()?;
        if parser.peek().is_some() {
            bail!("unexpected token after predicate");
        }
        Ok(Self {
            expression: expression.bind(schema)?,
        })
    }

    pub(crate) fn matches(
        &self,
        id: &NodeId,
        document: &Document,
        documents: &BTreeMap<NodeId, Document>,
    ) -> Result<bool> {
        self.expression.evaluate(id, document, documents)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompareOperator {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Word(String),
    Operator(CompareOperator),
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    Comma,
}

#[derive(Debug)]
enum RawExpression {
    Compare {
        field: String,
        operator: CompareOperator,
        value: String,
    },
    Contains {
        field: String,
        value: String,
    },
    In {
        field: String,
        values: Vec<String>,
    },
    Presence {
        field: String,
        expected: bool,
    },
    Any {
        field: String,
        predicate: Box<Self>,
    },
    Reaches {
        field: String,
        target: String,
    },
    Not(Box<Self>),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
}

impl RawExpression {
    fn bind(self, schema: &Schema) -> Result<Expression> {
        match self {
            Self::Compare {
                field,
                operator,
                value,
            } => {
                let field = ScalarField::bind(&field, schema)?;
                let value = field.parse_literal(&value, schema)?;
                Ok(Expression::Compare {
                    field,
                    operator,
                    value,
                })
            }
            Self::Contains { field, value } => {
                let field = SetField::bind(&field, schema)?;
                let value = field.parse_literal(&value, schema)?;
                Ok(Expression::Contains { field, value })
            }
            Self::In { field, values } => {
                let field = ScalarField::bind(&field, schema)?;
                let values = values
                    .iter()
                    .map(|value| field.parse_literal(value, schema))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expression::In { field, values })
            }
            Self::Presence { field, expected } => Ok(Expression::Presence {
                field: PresenceField::bind(&field, schema)?,
                expected,
            }),
            Self::Any { field, predicate } => Ok(Expression::Any {
                field: ReferenceAttribute::parse(&field, schema)?,
                predicate: Box::new(predicate.bind(schema)?),
            }),
            Self::Reaches { field, target } => Ok(Expression::Reaches {
                field: ReferenceAttribute::parse(&field, schema)?,
                target: NodeId::parse(&target)
                    .with_context(|| format!("invalid reaches target {target:?}"))?,
            }),
            Self::Not(expression) => Ok(Expression::Not(Box::new(expression.bind(schema)?))),
            Self::And(left, right) => Ok(Expression::And(
                Box::new(left.bind(schema)?),
                Box::new(right.bind(schema)?),
            )),
            Self::Or(left, right) => Ok(Expression::Or(
                Box::new(left.bind(schema)?),
                Box::new(right.bind(schema)?),
            )),
        }
    }
}

#[derive(Debug)]
enum Expression {
    Compare {
        field: ScalarField,
        operator: CompareOperator,
        value: ScalarValue,
    },
    Contains {
        field: SetField,
        value: String,
    },
    In {
        field: ScalarField,
        values: Vec<ScalarValue>,
    },
    Presence {
        field: PresenceField,
        expected: bool,
    },
    Any {
        field: ReferenceAttribute,
        predicate: Box<Self>,
    },
    Reaches {
        field: ReferenceAttribute,
        target: NodeId,
    },
    Not(Box<Self>),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
}

impl Expression {
    fn evaluate(
        &self,
        id: &NodeId,
        document: &Document,
        documents: &BTreeMap<NodeId, Document>,
    ) -> Result<bool> {
        match self {
            Self::Compare {
                field,
                operator,
                value,
            } => {
                let Some(actual) = field.value(id, document)? else {
                    return Ok(false);
                };
                Ok(operator.apply(actual.compare(value)))
            }
            Self::Contains { field, value } => Ok(field
                .values(document)
                .is_some_and(|set| set.contains(value))),
            Self::In { field, values } => {
                let Some(actual) = field.value(id, document)? else {
                    return Ok(false);
                };
                Ok(values.contains(&actual))
            }
            Self::Presence { field, expected } => Ok(field.is_present(id, document)? == *expected),
            Self::Any { field, predicate } => {
                for target in field.neighbor_ids(id, document, documents, false)? {
                    let target_document = documents
                        .get(&target)
                        .expect("neighbor_ids established that the target is loaded");
                    if predicate.evaluate(&target, target_document, documents)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Self::Reaches { field, target } => reaches(field, id, document, target, documents),
            Self::Not(expression) => Ok(!expression.evaluate(id, document, documents)?),
            Self::And(left, right) => Ok(left.evaluate(id, document, documents)?
                && right.evaluate(id, document, documents)?),
            Self::Or(left, right) => Ok(left.evaluate(id, document, documents)?
                || right.evaluate(id, document, documents)?),
        }
    }
}

impl CompareOperator {
    fn apply(self, ordering: Ordering) -> bool {
        match self {
            Self::Equal => ordering == Ordering::Equal,
            Self::NotEqual => ordering != Ordering::Equal,
            Self::Less => ordering == Ordering::Less,
            Self::LessEqual => ordering != Ordering::Greater,
            Self::Greater => ordering == Ordering::Greater,
            Self::GreaterEqual => ordering != Ordering::Less,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ScalarKind {
    String,
    Boolean,
    Integer,
    Enum,
    Reference,
}

#[derive(Debug)]
enum ScalarField {
    Id,
    NodeType,
    Title,
    Attribute { name: String, kind: ScalarKind },
}

impl ScalarField {
    fn bind(name: &str, schema: &Schema) -> Result<Self> {
        match name {
            "id" => Ok(Self::Id),
            "type" => Ok(Self::NodeType),
            "title" => Ok(Self::Title),
            _ => {
                let attribute = schema.attribute(name)?;
                let kind = match attribute.kind {
                    AttributeKind::String => ScalarKind::String,
                    AttributeKind::Boolean => ScalarKind::Boolean,
                    AttributeKind::Integer => ScalarKind::Integer,
                    AttributeKind::Enum(_) => ScalarKind::Enum,
                    AttributeKind::Reference(_) => ScalarKind::Reference,
                    AttributeKind::Set(_) => {
                        bail!("field {name} is set-valued, not scalar")
                    }
                };
                Ok(Self::Attribute {
                    name: name.to_owned(),
                    kind,
                })
            }
        }
    }

    fn parse_literal(&self, input: &str, schema: &Schema) -> Result<ScalarValue> {
        match self {
            Self::Id => Ok(ScalarValue::String(
                NodeId::parse(input)
                    .with_context(|| format!("invalid value for field id: {input:?}"))?
                    .to_string(),
            )),
            Self::NodeType | Self::Title => Ok(ScalarValue::String(input.to_owned())),
            Self::Attribute { name, kind } => {
                let value = schema.parse_scalar_literal(name, input)?;
                if matches!(kind, ScalarKind::Reference) {
                    NodeId::parse(input)
                        .with_context(|| format!("field {name} expects a node ID"))?;
                }
                ScalarValue::from_stored(*kind, name, &value)
            }
        }
    }

    fn value(&self, id: &NodeId, document: &Document) -> Result<Option<ScalarValue>> {
        match self {
            Self::Id => Ok(Some(ScalarValue::String(id.to_string()))),
            Self::NodeType => Ok(Some(ScalarValue::String(document.node_type.clone()))),
            Self::Title => Ok(Some(ScalarValue::String(document.title()?))),
            Self::Attribute { name, kind } => document
                .attributes
                .get(name)
                .map(|value| ScalarValue::from_stored(*kind, name, value))
                .transpose(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ScalarValue {
    String(String),
    Boolean(bool),
    Integer(i64),
    Enum(String),
    Reference(String),
}

impl ScalarValue {
    fn from_stored(kind: ScalarKind, field: &str, value: &Value) -> Result<Self> {
        match (kind, value) {
            (ScalarKind::String, Value::String(value)) => Ok(Self::String(value.clone())),
            (ScalarKind::Boolean, Value::Boolean(value)) => Ok(Self::Boolean(*value)),
            (ScalarKind::Integer, Value::Integer(value)) => Ok(Self::Integer(*value)),
            (ScalarKind::Enum, Value::Enum(value)) => Ok(Self::Enum(value.clone())),
            (ScalarKind::Reference, Value::Reference(value)) => Ok(Self::Reference(value.clone())),
            _ => bail!("stored value for field {field} disagrees with its schema type"),
        }
    }

    fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::String(left), Self::String(right))
            | (Self::Enum(left), Self::Enum(right))
            | (Self::Reference(left), Self::Reference(right)) => left.cmp(right),
            (Self::Boolean(left), Self::Boolean(right)) => left.cmp(right),
            (Self::Integer(left), Self::Integer(right)) => left.cmp(right),
            _ => unreachable!("binding gives both operands the same scalar type"),
        }
    }
}

#[derive(Debug)]
struct SetField {
    name: String,
    kind: SetKind,
}

impl SetField {
    fn bind(name: &str, schema: &Schema) -> Result<Self> {
        let attribute = schema.attribute(name)?;
        let AttributeKind::Set(kind) = &attribute.kind else {
            bail!("field {name} is scalar, not set-valued");
        };
        Ok(Self {
            name: name.to_owned(),
            kind: kind.clone(),
        })
    }

    fn parse_literal(&self, input: &str, schema: &Schema) -> Result<String> {
        let mut values = schema.parse_set_members(&self.name, input)?;
        let value = values
            .pop_first()
            .expect("a bare query token is not an empty set member");
        if matches!(self.kind, SetKind::Reference(_)) {
            NodeId::parse(&value)
                .with_context(|| format!("field {} expects a node ID", self.name))?;
        }
        Ok(value)
    }

    fn values<'a>(&self, document: &'a Document) -> Option<&'a BTreeSet<String>> {
        match document.attributes.get(&self.name) {
            Some(Value::Set(values)) => Some(values),
            None => None,
            Some(_) => unreachable!("document parsing follows the schema's set type"),
        }
    }
}

#[derive(Debug)]
enum PresenceField {
    Scalar(ScalarField),
    Set(SetField),
}

impl PresenceField {
    fn bind(name: &str, schema: &Schema) -> Result<Self> {
        if matches!(name, "id" | "type" | "title") {
            return Ok(Self::Scalar(ScalarField::bind(name, schema)?));
        }
        match &schema.attribute(name)?.kind {
            AttributeKind::Set(_) => Ok(Self::Set(SetField::bind(name, schema)?)),
            _ => Ok(Self::Scalar(ScalarField::bind(name, schema)?)),
        }
    }

    fn is_present(&self, id: &NodeId, document: &Document) -> Result<bool> {
        match self {
            Self::Scalar(ScalarField::Attribute { name, .. }) => {
                Ok(match document.attributes.get(name) {
                    Some(Value::String(value))
                    | Some(Value::Enum(value))
                    | Some(Value::Reference(value)) => !value.is_empty(),
                    Some(Value::Boolean(_) | Value::Integer(_)) => true,
                    Some(Value::Set(_)) => {
                        unreachable!("binding classified this as a scalar field")
                    }
                    None => false,
                })
            }
            Self::Scalar(field) => Ok(field.value(id, document)?.is_some()),
            Self::Set(field) => Ok(field
                .values(document)
                .is_some_and(|values| !values.is_empty())),
        }
    }
}

fn reaches(
    field: &ReferenceAttribute,
    start: &NodeId,
    document: &Document,
    target: &NodeId,
    documents: &BTreeMap<NodeId, Document>,
) -> Result<bool> {
    let mut pending = field.neighbor_ids(start, document, documents, false)?;
    let mut visited = BTreeSet::new();
    while let Some(next) = pending.pop_first() {
        let next_document = documents
            .get(&next)
            .expect("neighbor_ids established that the target is loaded");
        if &next == target {
            return Ok(true);
        }
        if visited.insert(next.clone()) {
            pending.extend(field.neighbor_ids(&next, next_document, documents, false)?);
        }
    }
    Ok(false)
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn parse_expression(&mut self) -> Result<RawExpression> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<RawExpression> {
        let mut expression = self.parse_and()?;
        while self.consume_word("or") {
            expression = RawExpression::Or(Box::new(expression), Box::new(self.parse_and()?));
        }
        Ok(expression)
    }

    fn parse_and(&mut self) -> Result<RawExpression> {
        let mut expression = self.parse_not()?;
        while self.consume_word("and") {
            expression = RawExpression::And(Box::new(expression), Box::new(self.parse_not()?));
        }
        Ok(expression)
    }

    fn parse_not(&mut self) -> Result<RawExpression> {
        if self.consume_word("not") {
            Ok(RawExpression::Not(Box::new(self.parse_not()?)))
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<RawExpression> {
        if self.consume(&Token::LeftParen) {
            let expression = self.parse_expression()?;
            self.expect(Token::RightParen)?;
            return Ok(expression);
        }
        if self.consume_word("has") {
            return Ok(RawExpression::Presence {
                field: self.expect_word()?,
                expected: true,
            });
        }
        if self.consume_word("no") {
            return Ok(RawExpression::Presence {
                field: self.expect_word()?,
                expected: false,
            });
        }
        if self.consume_word("any") {
            let field = self.expect_word()?;
            self.expect(Token::LeftParen)?;
            let predicate = self.parse_expression()?;
            self.expect(Token::RightParen)?;
            return Ok(RawExpression::Any {
                field,
                predicate: Box::new(predicate),
            });
        }
        if self.consume_word("reaches") {
            return Ok(RawExpression::Reaches {
                field: self.expect_word()?,
                target: self.expect_word()?,
            });
        }

        let field = self.expect_word()?;
        if self.consume_word("contain") {
            return Ok(RawExpression::Contains {
                field,
                value: self.expect_word()?,
            });
        }
        if self.consume_word("in") {
            return Ok(RawExpression::In {
                field,
                values: self.parse_list()?,
            });
        }
        Ok(RawExpression::Compare {
            field,
            operator: self.expect_operator()?,
            value: self.expect_word()?,
        })
    }

    fn parse_list(&mut self) -> Result<Vec<String>> {
        self.expect(Token::LeftBracket)?;
        let mut values = Vec::new();
        loop {
            values.push(self.expect_word()?);
            if self.consume(&Token::Comma) {
                continue;
            }
            self.expect(Token::RightBracket)?;
            return Ok(values);
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn consume(&mut self, token: &Token) -> bool {
        if self.peek() == Some(token) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn consume_word(&mut self, expected: &str) -> bool {
        if matches!(self.peek(), Some(Token::Word(word)) if word == expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: Token) -> Result<()> {
        if self.consume(&expected) {
            Ok(())
        } else {
            bail!("expected {expected:?}")
        }
    }

    fn expect_word(&mut self) -> Result<String> {
        match self.peek() {
            Some(Token::Word(word)) => {
                let word = word.clone();
                self.position += 1;
                Ok(word)
            }
            _ => bail!("expected bare token"),
        }
    }

    fn expect_operator(&mut self) -> Result<CompareOperator> {
        match self.peek() {
            Some(Token::Operator(operator)) => {
                let operator = *operator;
                self.position += 1;
                Ok(operator)
            }
            _ => bail!("expected comparison operator"),
        }
    }
}

fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut characters = input.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(character) = characters.peek().copied() {
        match character {
            character if character.is_whitespace() => {
                characters.next();
            }
            '(' => {
                characters.next();
                tokens.push(Token::LeftParen);
            }
            ')' => {
                characters.next();
                tokens.push(Token::RightParen);
            }
            '[' => {
                characters.next();
                tokens.push(Token::LeftBracket);
            }
            ']' => {
                characters.next();
                tokens.push(Token::RightBracket);
            }
            ',' => {
                characters.next();
                tokens.push(Token::Comma);
            }
            '=' => {
                characters.next();
                tokens.push(Token::Operator(CompareOperator::Equal));
            }
            '!' => {
                characters.next();
                if characters.next() == Some('=') {
                    tokens.push(Token::Operator(CompareOperator::NotEqual));
                } else {
                    bail!("expected != operator");
                }
            }
            '<' => {
                characters.next();
                if characters.peek() == Some(&'=') {
                    characters.next();
                    tokens.push(Token::Operator(CompareOperator::LessEqual));
                } else {
                    tokens.push(Token::Operator(CompareOperator::Less));
                }
            }
            '>' => {
                characters.next();
                if characters.peek() == Some(&'=') {
                    characters.next();
                    tokens.push(Token::Operator(CompareOperator::GreaterEqual));
                } else {
                    tokens.push(Token::Operator(CompareOperator::Greater));
                }
            }
            _ => {
                let mut word = String::new();
                while let Some(next) = characters.peek().copied() {
                    if next.is_whitespace() || "()[],=!<>".contains(next) {
                        break;
                    }
                    word.push(next);
                    characters.next();
                }
                if word.is_empty() {
                    bail!("invalid token");
                }
                tokens.push(Token::Word(word));
            }
        }
    }
    if tokens.is_empty() {
        bail!("empty predicate");
    }
    Ok(tokens)
}
