use eyre::Result;
use regex::{Captures, Match, Regex};

fn main() {
    println!("Hello, world!");
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Query(String);

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LayeredQueries(Vec<LayeredQuery>);

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LayeredQuery {
    Query(Query),
    Bracket(LayeredQueries),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Condition {
    Keyword(String),
    ExactKeyword(String),
    Not(Box<Condition>),
    And(Vec<Condition>),
    Or(Vec<Condition>),
}

impl Query {
    pub fn new(value: String) -> Self {
        Query(
            value
                .replace("（", "(")
                .replace("）", ")")
                .replace("”", "\"")
                .replace("　", " "),
        )
    }

    pub fn value(&self) -> &str {
        self.0.as_str()
    }

    fn is_not_blank(&self) -> bool {
        self.0.replace(" ", "").is_empty() == false
    }

    fn layered_by_bracket(self) -> Result<LayeredQueries> {
        fn filter_not_blank_query(regex_match: Option<Match>) -> Option<Query> {
            regex_match
                .map(|m| Query::new(m.as_str().into()))
                .filter(|q| q.is_not_blank())
        }

        fn pick_layer_by_bracket(
            query: String, bracket_queries: &mut Vec<Query>,
        ) -> Result<String> {
            let regex_bracket = Regex::new(r"\(([^\(\)]*)\)")?;
            let innermost_bracket_removed_query = regex_bracket
                .replace_all(
                    query.as_str(),
                    |captures: &Captures| match filter_not_blank_query(captures.get(1)) {
                        Some(q) => {
                            bracket_queries.push(q);
                            format!("（{}）", bracket_queries.len())
                        }
                        None => String::from(""),
                    },
                )
                .to_string();
            match query != innermost_bracket_removed_query {
                true => pick_layer_by_bracket(innermost_bracket_removed_query, bracket_queries),
                false => Ok(query),
            }
        }

        let mut bracket_queries = Vec::<Query>::new();
        let all_brackets_removed_query = pick_layer_by_bracket(self.0, &mut bracket_queries)?;

        fn combine_layered_query(
            query: Query, bracket_queries: &Vec<Query>,
        ) -> Result<LayeredQueries> {
            let regex_layered_by_bracket = Regex::new(r"([^\(\)]*)\((\d)\)([^\(\)]*)")?;
            let mut layered_queries = Vec::<LayeredQuery>::new();
            regex_layered_by_bracket
                .captures_iter(query.value())
                .for_each(|captures| {
                    filter_not_blank_query(captures.get(1))
                        .map(|q| layered_queries.push(LayeredQuery::Query(q)));
                    captures
                        .get(2)
                        .map(|m| m.as_str().parse::<usize>())
                        .map(|index| {
                            index.map(|i| {
                                bracket_queries.get(i - 1).map(|q: &Query| {
                                    combine_layered_query(q.clone(), bracket_queries)
                                        .map(|v| layered_queries.push(LayeredQuery::Bracket(v)))
                                })
                            })
                        });
                    filter_not_blank_query(captures.get(3))
                        .map(|q| layered_queries.push(LayeredQuery::Query(q)));
                });
            if layered_queries.is_empty() {
                layered_queries.push(LayeredQuery::Query(query))
            }
            Ok(LayeredQueries(layered_queries))
        }

        Ok(combine_layered_query(
            Query::new(all_brackets_removed_query),
            &bracket_queries,
        )?)
    }
}

impl LayeredQueries {
    // k1 or (k2 and (-k3 or -k4))
    fn parse(self) -> Result<Condition> {
        self.0.into_iter().for_each(|layer| {});
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_full_width_bracket_quotation_and_space_when_new() {
        let target =
            Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　（（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　”あああ　いいい”　ううう））　”　ＪＪＪ　”　（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　".into());
        assert_eq!(
            target.value(),
            " ＡＡＡ (\"１１１ ＣＣＣ\" (( ＤＤＤ エエエ ) ＦＦＦ) ＧＧＧ (ＨＨＨ \"あああ いいい\" ううう)) \" ＪＪＪ \" (ＫＫＫ ( ) ＬＬＬ)  (ＭＭＭ) ２２２ "
        )
    }

    #[test]
    fn test_layered_by_bracket() {
        let target =
            Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　（（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　”あああ　いいい”　ううう））　”　ＪＪＪ　”　（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　".into());
        assert_eq!(
            target.layered_by_bracket().unwrap(),
            LayeredQueries(vec![
                LayeredQuery::Query(Query::new(" ＡＡＡ ".into())),
                LayeredQuery::Bracket(LayeredQueries(vec![
                    LayeredQuery::Query(Query::new("\"１１１ ＣＣＣ\" ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![
                        LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(
                            Query::new(" ＤＤＤ エエエ ".into())
                        ),])),
                        LayeredQuery::Query(Query::new(" ＦＦＦ".into())),
                    ])),
                    LayeredQuery::Query(Query::new(" ＧＧＧ ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                        "ＨＨＨ \"あああ いいい\" ううう".into()
                    )),]))
                ])),
                LayeredQuery::Query(Query::new(" \" ＪＪＪ \" ".into())),
                LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                    "ＫＫＫ  ＬＬＬ".into()
                )),])),
                LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                    "ＭＭＭ".into()
                )),])),
                LayeredQuery::Query(Query::new(" ２２２ ".into())),
            ])
        )
    }

    #[test]
    fn test_parse() {
        let target =
            Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　（（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　”あああ　いいい”　ううう））　”　ＪＪＪ　”　（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　".into());
        assert_eq!(
            target.layered_by_bracket().unwrap().parse().unwrap(),
            Condition::Or(vec![
                Condition::Keyword("ＡＡＡ".into()),
                Condition::Or(vec![
                    Condition::ExactKeyword("１１１ ＣＣＣ".into()),
                    Condition::Or(vec![
                        Condition::Or(vec![
                            Condition::Keyword("ＤＤＤ".into()),
                            Condition::Keyword("エエエ".into()),
                        ]),
                        Condition::Keyword("ＦＦＦ".into()),
                    ]),
                    Condition::Keyword("ＧＧＧ".into()),
                    Condition::Or(vec![
                        Condition::Keyword("ＨＨＨ".into()),
                        Condition::ExactKeyword("あああ いいい".into()),
                        Condition::Keyword("ううう".into()),
                    ]),
                ]),
                Condition::ExactKeyword(" ＪＪＪ ".into()),
                Condition::Or(vec![
                    Condition::Keyword("ＫＫＫ".into()),
                    Condition::Keyword("ＬＬＬ".into()),
                ]),
                Condition::Or(vec![Condition::Keyword("ＭＭＭ".into()),]),
                Condition::Keyword("２２２".into()),
            ])
        )
    }
}
