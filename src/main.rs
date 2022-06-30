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
    NegativeBracket(LayeredQueries),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Condition {
    Keyword(String),
    ExactKeyword(String),
    Not(Box<Condition>),
    Operator(Operator, Vec<Condition>),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Operator {
    And,
    Or,
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
        self.value().replace(" ", "").is_empty() == false
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
        let all_brackets_replace_query = pick_layer_by_bracket(self.0, &mut bracket_queries)?;

        fn combine_layered_query(
            query: Query, bracket_queries: &Vec<Query>,
        ) -> Result<LayeredQueries> {
            let regex_layered_by_bracket = Regex::new(r"([^\(\)]*)\((\d)\)")?;
            let mut layered_queries = Vec::<LayeredQuery>::new();
            let the_last_query_after_all_brackets = regex_layered_by_bracket
                .replace_all(query.value(), |captures: &Captures| {
                    let mut is_negative_bracket = false;
                    filter_not_blank_query(captures.get(1)).map(|mut q| {
                        if q.value().ends_with("-") {
                            is_negative_bracket = true;
                            q = Query::new(String::from(&q.value()[0..q.value().len() - 1]))
                        }
                        if q.is_not_blank() {
                            layered_queries.push(LayeredQuery::Query(q))
                        }
                    });
                    captures
                        .get(2)
                        .map(|m| m.as_str().parse::<usize>())
                        .map(|index| {
                            index.map(|i| {
                                bracket_queries.get(i - 1).map(|q: &Query| {
                                    combine_layered_query(q.clone(), bracket_queries).map(|v| {
                                        layered_queries.push(if is_negative_bracket {
                                            LayeredQuery::NegativeBracket(v)
                                        } else {
                                            LayeredQuery::Bracket(v)
                                        })
                                    })
                                })
                            })
                        });
                    String::from("")
                })
                .to_string();
            let the_last_query = Query::new(the_last_query_after_all_brackets);
            if the_last_query.is_not_blank() {
                layered_queries.push(LayeredQuery::Query(the_last_query))
            }
            Ok(LayeredQueries(layered_queries))
        }

        Ok(combine_layered_query(
            Query::new(all_brackets_replace_query),
            &bracket_queries,
        )?)
    }

    fn parse_to_condition(self) -> Result<Condition> {
        unimplemented!()
    }
}

impl LayeredQueries {
    // k1 or (k2 and (-k3 or -k4))
    fn parse_to_condition(self) -> Result<Condition> {
        let layered_query_count = self.0.iter().count();
        self.0
            .into_iter()
            .enumerate()
            .for_each(|(index, layered_query)| match index {
                i if i < layered_query_count - 1 => {}
                _ => {}
            });
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_full_width_bracket_quotation_and_space_when_new() {
        let target =
            Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　（-（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　-”あああ　いいい”　ううう））　”　ＪＪＪ　”　-（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　".into());
        assert_eq!(
            target.value(),
            " ＡＡＡ (\"１１１ ＣＣＣ\" (-( ＤＤＤ エエエ ) ＦＦＦ) ＧＧＧ (ＨＨＨ -\"あああ いいい\" ううう)) \" ＪＪＪ \" -(ＫＫＫ ( ) ＬＬＬ)  (ＭＭＭ) ２２２ "
        )
    }

    #[test]
    fn test_layered_by_bracket() {
        let target =
            Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　（-（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　-”あああ　いいい”　ううう））　”　ＪＪＪ　”　-（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　".into());
        assert_eq!(
            target.layered_by_bracket().unwrap(),
            LayeredQueries(vec![
                LayeredQuery::Query(Query::new(" ＡＡＡ ".into())),
                LayeredQuery::Bracket(LayeredQueries(vec![
                    LayeredQuery::Query(Query::new("\"１１１ ＣＣＣ\" ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![
                        LayeredQuery::NegativeBracket(LayeredQueries(vec![LayeredQuery::Query(
                            Query::new(" ＤＤＤ エエエ ".into())
                        ),])),
                        LayeredQuery::Query(Query::new(" ＦＦＦ".into())),
                    ])),
                    LayeredQuery::Query(Query::new(" ＧＧＧ ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                        "ＨＨＨ -\"あああ いいい\" ううう".into()
                    )),]))
                ])),
                LayeredQuery::Query(Query::new(" \" ＪＪＪ \" ".into())),
                LayeredQuery::NegativeBracket(LayeredQueries(vec![LayeredQuery::Query(
                    Query::new("ＫＫＫ  ＬＬＬ".into())
                ),])),
                LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                    "ＭＭＭ".into()
                )),])),
                LayeredQuery::Query(Query::new(" ２２２ ".into())),
            ])
        )
    }

    #[test]
    fn test_query_parse_to_condition() {
        let target = Query::new("　ＡＡＡ　and　-ＢＢＢ　ＣＣＣ　or　ＤＤＤ　and　\"　ＥＥＥ　and　ＦＦＦ　\"　and　-\"　ＧＧＧ　or　ＨＨＨ　\"　ＩＩＩ　".into());
        assert_eq!(
            target.parse_to_condition().unwrap(),
            Condition::Operator(
                Operator::Or,
                vec![
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Keyword("ＡＡＡ".into()),
                            Condition::Not(Box::new(Condition::Keyword("ＢＢＢ".into())))
                        ]
                    ),
                    Condition::Keyword("ＣＣＣ".into()),
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Keyword("ＤＤＤ".into()),
                            Condition::ExactKeyword("ＥＥＥ and ＦＦＦ".into()),
                            Condition::Not(Box::new(Condition::ExactKeyword(
                                "ＧＧＧ or ＨＨＨ".into()
                            )))
                        ]
                    ),
                    Condition::Keyword("ＩＩＩ".into()),
                ]
            )
        )
    }

    #[test]
    fn test_layered_queries_parse_to_condition() {
        let target =
            Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　（-（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　-”あああ　いいい”　ううう））　”　ＪＪＪ　”　-（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　".into());
        assert_eq!(
            target
                .layered_by_bracket()
                .unwrap()
                .parse_to_condition()
                .unwrap(),
            Condition::Operator(
                Operator::Or,
                vec![
                    Condition::Keyword("ＡＡＡ".into()),
                    Condition::Operator(
                        Operator::Or,
                        vec![
                            Condition::ExactKeyword("１１１ ＣＣＣ".into()),
                            Condition::Operator(
                                Operator::Or,
                                vec![
                                    Condition::Not(Box::new(Condition::Operator(
                                        Operator::Or,
                                        vec![
                                            Condition::Keyword("ＤＤＤ".into()),
                                            Condition::Keyword("エエエ".into()),
                                        ]
                                    ))),
                                    Condition::Keyword("ＦＦＦ".into()),
                                ]
                            ),
                            Condition::Keyword("ＧＧＧ".into()),
                            Condition::Operator(
                                Operator::Or,
                                vec![
                                    Condition::Keyword("ＨＨＨ".into()),
                                    Condition::Not(Box::new(Condition::ExactKeyword(
                                        "あああ いいい".into()
                                    ))),
                                    Condition::Keyword("ううう".into()),
                                ]
                            ),
                        ]
                    ),
                    Condition::ExactKeyword(" ＪＪＪ ".into()),
                    Condition::Not(Box::new(Condition::Operator(
                        Operator::Or,
                        vec![
                            Condition::Keyword("ＫＫＫ".into()),
                            Condition::Keyword("ＬＬＬ".into()),
                        ]
                    ))),
                    Condition::Operator(Operator::Or, vec![Condition::Keyword("ＭＭＭ".into()),]),
                    Condition::Keyword("２２２".into()),
                ]
            )
        )
    }
}
