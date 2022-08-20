use crate::layered_query::LayeredQueries;
use crate::query::Query;

mod condition;
mod layered_query;
mod query;

fn main() {
    println!("Hello, world!");
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Operator {
    And,
    Or,
}
