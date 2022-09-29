use crate::regex_approach::query::Query;
use regex::Match;

mod condition;
pub(crate) mod layered_query;
pub(crate) mod query;

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
