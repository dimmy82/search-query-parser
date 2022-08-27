use crate::condition::Condition;
use crate::query::Query;
use crate::{filter_not_blank_query, match_to_number};
use eyre::Result;
use regex::{Captures, Regex};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LayeredQuery {
    Query(Query),
    Bracket(LayeredQueries),
    NegativeBracket(LayeredQueries),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LayeredQueries(Vec<LayeredQuery>);

impl LayeredQueries {
    pub(crate) fn parse(query: Query) -> Result<LayeredQueries> {
        let query = query.normalize();
        let mut bracket_queries = Vec::<Query>::new();
        let all_brackets_replace_query = Self::pick_layer_by_bracket(query, &mut bracket_queries)?;
        Ok(Self::combine_layered_query(
            all_brackets_replace_query,
            &bracket_queries,
        )?)
    }

    fn pick_layer_by_bracket(query: Query, bracket_queries: &mut Vec<Query>) -> Result<Query> {
        let regex_bracket = Regex::new(r"\(([^\(\)]*)\)")?;
        let innermost_bracket_removed_query = Query::new(
            regex_bracket
                .replace_all(
                    query.value_ref(),
                    |captures: &Captures| match filter_not_blank_query(captures.get(1)) {
                        Some(q) => {
                            bracket_queries.push(q);
                            format!("（{}）", bracket_queries.len())
                        }
                        None => String::from(""),
                    },
                )
                .into(),
        );
        match query == innermost_bracket_removed_query {
            false => Self::pick_layer_by_bracket(innermost_bracket_removed_query, bracket_queries),
            true => Ok(query),
        }
    }

    fn combine_layered_query(query: Query, bracket_queries: &Vec<Query>) -> Result<LayeredQueries> {
        let mut layered_queries = Vec::<LayeredQuery>::new();
        let regex_layered_by_bracket = Regex::new(r"([^（）]*)（(\d)）")?;
        let the_last_query_after_all_brackets = regex_layered_by_bracket
            .replace_all(query.value_ref(), |captures: &Captures| {
                let mut is_negative_bracket = false;
                filter_not_blank_query(captures.get(1)).map(|mut q| {
                    if q.value_ref().ends_with("-") {
                        is_negative_bracket = true;
                        q = Query::new(String::from(&q.value_ref()[0..q.value_ref().len() - 1]))
                    }
                    if q.is_not_blank() {
                        layered_queries.push(LayeredQuery::Query(q))
                    }
                });
                match_to_number(captures.get(2), |i| {
                    bracket_queries.get(i - 1).map(|q| {
                        Self::combine_layered_query(q.clone(), bracket_queries).map(|v| {
                            layered_queries.push(if is_negative_bracket {
                                LayeredQuery::NegativeBracket(v)
                            } else {
                                LayeredQuery::Bracket(v)
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
        Ok(Self(layered_queries))
    }

    // k1 or (k2 and (-k3 or -k4))
    pub(crate) fn to_condition(self) -> Result<Condition> {
        let _layered_query_count = self.0.iter().count();
        self.0
            .into_iter()
            .enumerate()
            .for_each(|(_index, layered_query)| match layered_query {
                LayeredQuery::Query(query) => {
                    let _ = query.to_condition();
                }
                _ => {}
            });
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::Query;
    use crate::Operator;

    impl Query {
        fn new_with_normalize(value: String) -> Self {
            Self::new(value).normalize()
        }
    }

    mod test_parse {
        use super::*;

        #[test]
        fn test_parse_without_bracket() {
            let query = Query::new(
                "　ＡＡＡ　”１１１　ＣＣＣ”　-ＤＤＤ　or　エエエ　and　ＦＦＦ　-”あああ　いいい”"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query.clone()).unwrap(),
                LayeredQueries(vec![LayeredQuery::Query(Query::new_with_normalize(
                    query.value()
                ))])
            )
        }

        #[test]
        fn test_parse_with_only_start_bracket() {
            let query = Query::new(
                "　ＡＡＡ　”１１１　ＣＣＣ”　-（ＤＤＤ　or　エエエ　and　（　ＦＦＦ　-”あああ　いいい”"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query.clone()).unwrap(),
                LayeredQueries(vec![LayeredQuery::Query(Query::new_with_normalize(
                    query.value()
                ))])
            )
        }

        #[test]
        fn test_parse_with_only_end_bracket() {
            let query = Query::new(
                "　ＡＡＡ　”１１１　ＣＣＣ”）　-ＤＤＤ　or　エエエ　）　and　ＦＦＦ　-”あああ　いいい”"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query.clone()).unwrap(),
                LayeredQueries(vec![LayeredQuery::Query(Query::new_with_normalize(
                    query.value()
                ))])
            )
        }

        #[test]
        fn test_parse_with_start_bracket_more_than_end_bracket() {
            let query = Query::new(
                "　ＡＡＡ　”１１１　ＣＣＣ”　-（ＤＤＤ　or　エエエ）　and　（　ＦＦＦ　-”あああ　いいい”"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query.clone()).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::Query(Query::new_with_normalize(
                        "　ＡＡＡ　”１１１　ＣＣＣ”　".into()
                    )),
                    LayeredQuery::NegativeBracket(LayeredQueries(vec![LayeredQuery::Query(
                        Query::new_with_normalize("ＤＤＤ　or　エエエ".into())
                    )])),
                    LayeredQuery::Query(Query::new_with_normalize(
                        "　and　（　ＦＦＦ　-”あああ　いいい”".into()
                    ))
                ])
            )
        }

        #[test]
        fn test_parse_with_start_bracket_less_than_end_bracket() {
            let query = Query::new(
                "　ＡＡＡ　”１１１　ＣＣＣ”　-（ＤＤＤ　or　エエエ）　and　）　ＦＦＦ　-”あああ　いいい”"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query.clone()).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::Query(Query::new_with_normalize(
                        "　ＡＡＡ　”１１１　ＣＣＣ”　".into()
                    )),
                    LayeredQuery::NegativeBracket(LayeredQueries(vec![LayeredQuery::Query(
                        Query::new_with_normalize("ＤＤＤ　or　エエエ".into())
                    )])),
                    LayeredQuery::Query(Query::new_with_normalize(
                        "　and　）　ＦＦＦ　-”あああ　いいい”".into()
                    ))
                ])
            )
        }

        #[test]
        fn test_parse_with_reverse_bracket() {
            let query = Query::new(
                "　ＡＡＡ）　”１１１　ＣＣＣ”　-（ＤＤＤ　or　エエエ）　and　（　ＦＦＦ　-”あああ　いいい”"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query.clone()).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::Query(Query::new_with_normalize(
                        "　ＡＡＡ）　”１１１　ＣＣＣ”　".into()
                    )),
                    LayeredQuery::NegativeBracket(LayeredQueries(vec![LayeredQuery::Query(
                        Query::new_with_normalize("ＤＤＤ　or　エエエ".into())
                    )])),
                    LayeredQuery::Query(Query::new_with_normalize(
                        "　and　（　ＦＦＦ　-”あああ　いいい”".into()
                    ))
                ])
            )
        }

        #[test]
        fn test_parse_full_pattern() {
            let query =
                Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　（-（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　-”あああ　いいい”　ううう））　”　ＪＪＪ　”　-（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　".into());
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::Query(Query::new_with_normalize(" ＡＡＡ ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![
                        LayeredQuery::Query(Query::new_with_normalize("\"１１１ ＣＣＣ\" ".into())),
                        LayeredQuery::Bracket(LayeredQueries(vec![
                            LayeredQuery::NegativeBracket(LayeredQueries(vec![
                                LayeredQuery::Query(Query::new_with_normalize(
                                    " ＤＤＤ エエエ ".into()
                                )),
                            ])),
                            LayeredQuery::Query(Query::new_with_normalize(" ＦＦＦ".into())),
                        ])),
                        LayeredQuery::Query(Query::new_with_normalize(" ＧＧＧ ".into())),
                        LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(
                            Query::new_with_normalize("ＨＨＨ -\"あああ いいい\" ううう".into())
                        ),]))
                    ])),
                    LayeredQuery::Query(Query::new_with_normalize(" \" ＪＪＪ \" ".into())),
                    LayeredQuery::NegativeBracket(LayeredQueries(vec![LayeredQuery::Query(
                        Query::new_with_normalize("ＫＫＫ  ＬＬＬ".into())
                    ),])),
                    LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(
                        Query::new_with_normalize("ＭＭＭ".into())
                    ),])),
                    LayeredQuery::Query(Query::new_with_normalize(" ２２２ ".into())),
                ])
            )
        }
    }

    #[test]
    fn test_layered_queries_parse_to_condition() {
        let query =
            Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　（-（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　-”あああ　いいい”　ううう））　”　ＪＪＪ　”　-（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　".into());
        assert_eq!(
            LayeredQueries::parse(query)
                .unwrap()
                .to_condition()
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
                                    Condition::Negative(Box::new(Condition::Operator(
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
                                    Condition::Negative(Box::new(Condition::ExactKeyword(
                                        "あああ いいい".into()
                                    ))),
                                    Condition::Keyword("ううう".into()),
                                ]
                            ),
                        ]
                    ),
                    Condition::ExactKeyword(" ＪＪＪ ".into()),
                    Condition::Negative(Box::new(Condition::Operator(
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
