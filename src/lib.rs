mod bnf_approach;
mod regex_approach;

use crate::regex_approach::layered_query::LayeredQueries;
use crate::regex_approach::query::Query;
use eyre::Result;
use serde::Serialize;

pub fn parse_query_to_condition(query: &str) -> Result<Condition> {
    LayeredQueries::parse(Query::new(query.into()))?.to_condition()
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum Condition {
    None,
    Keyword(String),
    PhraseKeyword(String),
    Not(Box<Condition>),
    Operator(Operator, Vec<Condition>),
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum Operator {
    And,
    Or,
}
