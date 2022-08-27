use crate::layered_query::LayeredQueries;
use crate::query::Query;
use regex::Match;

mod condition;
mod layered_query;
mod query;

fn main() {
    let _result = LayeredQueries::parse(Query::new("search".into()))
        .map(|layered_queries| layered_queries.to_condition());
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Operator {
    And,
    Or,
}

pub(crate) fn filter_not_blank_query(regex_match: Option<Match>) -> Option<Query> {
    regex_match
        .map(|m| Query::new(m.as_str().into()))
        .filter(|q| q.is_not_blank())
}

pub(crate) fn match_to_number<F: FnOnce(usize) -> Option<R>, R>(
    regex_match: Option<Match>, call_back: F,
) -> Option<R> {
    regex_match
        .map(|m| m.as_str().parse::<usize>())
        .map(|index| index.map(|i| call_back(i)).unwrap_or(None))
        .flatten()
}
