use crate::layered_queries::LayeredQueries;
use crate::query::Query;

mod layered_queries;
mod query;

fn main() {
    println!("Hello, world!");
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LayeredQuery {
    Query(Query),
    Bracket(LayeredQueries),
    NegativeBracket(LayeredQueries),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Condition {
    Keyword(String),
    ExactKeyword(String),
    Negative(Box<Condition>),
    Operator(Operator, Vec<Condition>),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Operator {
    And,
    Or,
}
