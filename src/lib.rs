use crate::condition::Condition;
use crate::layered_query::LayeredQueries;
use crate::query::Query;
use eyre::Result;
use regex::Match;
mod condition;
mod layered_query;
mod query;

pub fn parse_query_to_condition(query: &str) -> Result<Condition> {
    LayeredQueries::parse(Query::new(query.into()))?.to_condition()
}

pub(crate) fn regex_match_not_blank_query(regex_match: Option<Match>) -> Option<Query> {
    regex_match
        .map(|m| Query::new(m.as_str().into()))
        .filter(|q| q.is_not_blank())
}

pub(crate) fn regex_match_number<F: FnOnce(usize) -> Option<R>, R>(
    regex_match: Option<Match>, call_back: F,
) -> Option<R> {
    regex_match
        .map(|m| m.as_str().parse::<usize>())
        .map(|index| index.map(|i| call_back(i)).unwrap_or(None))
        .flatten()
}
