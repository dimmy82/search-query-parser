use crate::regex_approach::query::Query;
use crate::regex_approach::{regex_match_not_blank_query, regex_match_number};
use crate::{Condition, Operator};
use eyre::Result;
use regex::{Captures, Regex};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum LayeredQuery {
    Query(Query),
    Bracket(LayeredQueries),
    NegativeBracket(LayeredQueries),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct LayeredQueries(Vec<LayeredQuery>);

impl LayeredQueries {
    pub(crate) fn parse(query: Query) -> Result<LayeredQueries> {
        let (query, negative_phrase_keywords, phrase_keywords) = query
            .normalize_double_quotation()
            .extract_phrase_keywords()?;
        let query = query.normalize_symbols_except_double_quotation();
        let mut bracket_queries = Vec::<Query>::new();
        let all_brackets_picked_query = Self::pick_layer_by_bracket(query, &mut bracket_queries)?;
        Ok(Self::combine_layered_query(
            all_brackets_picked_query,
            &bracket_queries,
            &negative_phrase_keywords,
            &phrase_keywords,
        )?)
    }

    fn pick_layer_by_bracket(query: Query, bracket_queries: &mut Vec<Query>) -> Result<Query> {
        let regex_bracket = Regex::new(r"\(([^()]*)\)")?;
        let innermost_bracket_removed_query = Query::new(
            regex_bracket
                .replace_all(query.value_ref(), |captures: &Captures| {
                    match regex_match_not_blank_query(captures.get(1)) {
                        Some(q) => {
                            bracket_queries.push(q);
                            format!("（{}）", bracket_queries.len())
                        }
                        None => String::from(""),
                    }
                })
                .into(),
        );
        match query == innermost_bracket_removed_query {
            false => Self::pick_layer_by_bracket(innermost_bracket_removed_query, bracket_queries),
            true => Ok(query.remove_bracket()),
        }
    }

    fn combine_layered_query(
        query: Query, bracket_queries: &Vec<Query>, negative_phrase_keywords: &Vec<Query>,
        phrase_keywords: &Vec<Query>,
    ) -> Result<LayeredQueries> {
        let regex_layered_by_bracket = Regex::new(r"([^（）]*)（(\d+)）")?;
        let mut layered_queries = Vec::<LayeredQuery>::new();
        let the_last_query_after_all_brackets = regex_layered_by_bracket
            .replace_all(query.value_ref(), |captures: &Captures| {
                let mut is_negative_bracket = false;
                regex_match_not_blank_query(captures.get(1)).map(|mut q| {
                    if q.value_ref().ends_with("-") {
                        is_negative_bracket = true;
                        q = Query::new(String::from(&q.value_ref()[0..q.value_ref().len() - 1]))
                    }
                    let _ = q
                        .combine_phrase_keywords(negative_phrase_keywords, phrase_keywords)
                        .map(|q| {
                            if q.is_not_blank() {
                                layered_queries.push(LayeredQuery::Query(q))
                            }
                        });
                });
                regex_match_number(captures.get(2), |i| {
                    bracket_queries.get(i - 1).map(|q| {
                        Self::combine_layered_query(
                            q.clone(),
                            bracket_queries,
                            negative_phrase_keywords,
                            phrase_keywords,
                        )
                        .map(|lqs| {
                            layered_queries.push(if is_negative_bracket {
                                LayeredQuery::NegativeBracket(lqs)
                            } else {
                                LayeredQuery::Bracket(lqs)
                            })
                        })
                    })
                });
                String::from("")
            })
            .to_string();
        let the_last_query = Query::new(the_last_query_after_all_brackets)
            .combine_phrase_keywords(negative_phrase_keywords, phrase_keywords)?;
        if the_last_query.is_not_blank() {
            layered_queries.push(LayeredQuery::Query(the_last_query))
        }
        Ok(Self(layered_queries))
    }

    pub(crate) fn to_condition(self) -> Result<Condition> {
        let mut query_string = String::new();
        let mut conditions = Vec::<Condition>::new();

        for layered_query in self.0 {
            match layered_query {
                LayeredQuery::Query(query) => {
                    let (is_start_with_or, condition, is_end_with_or) = query.to_condition()?;
                    query_string.push_str(
                        format!(
                            " {} {} {} ",
                            if is_start_with_or { "or" } else { "and" },
                            conditions.len(),
                            if is_end_with_or { "or" } else { "and" }
                        )
                        .as_str(),
                    );
                    conditions.push(condition);
                }
                LayeredQuery::Bracket(layered_queries) => {
                    let condition = layered_queries.to_condition()?;
                    query_string.push_str(format!(" {} ", conditions.len()).as_str());
                    conditions.push(condition);
                }
                LayeredQuery::NegativeBracket(layered_queries) => {
                    let condition = layered_queries.to_condition()?;
                    query_string.push_str(format!(" {} ", conditions.len()).as_str());
                    conditions.push(Condition::Not(Box::new(condition)));
                }
            }
        }

        let query = Query::new(query_string);
        let (_, condition, _) = query.to_condition()?;
        let condition = match condition {
            Condition::Keyword(index) => Self::get_condition(index, &conditions)?,
            Condition::Operator(operator, layer1_conditions) => {
                let mut real_layer1_conditions = Vec::<Condition>::new();
                for condition in layer1_conditions {
                    real_layer1_conditions.push(match condition {
                        Condition::Keyword(index) => Self::get_condition(index, &conditions)?,
                        Condition::Operator(Operator::And, layer2_conditions) => {
                            let mut real_layer2_conditions = Vec::<Condition>::new();
                            for condition in layer2_conditions {
                                real_layer2_conditions.push(match condition {
                                    Condition::Keyword(index) => {
                                        Self::get_condition(index, &conditions)?
                                    }
                                    _ => Condition::None,
                                })
                            }
                            Condition::Operator(Operator::And, real_layer2_conditions)
                        }
                        _ => Condition::None,
                    })
                }
                Condition::Operator(operator, real_layer1_conditions)
            }
            _ => Condition::None,
        };
        Ok(condition.simplify())
    }

    fn get_condition(index: String, conditions: &Vec<Condition>) -> Result<Condition> {
        Ok(conditions
            .get(index.parse::<usize>()?)
            .map(|condition: &Condition| condition.clone())
            .unwrap_or(Condition::None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod test_parse {
        use super::*;

        #[test]
        fn test_parse_without_bracket() {
            let query = Query::new(
                "　ＡＡＡ　”１１１　ＣＣＣ”　-ＤＤＤ　or　エエエ　and　ＦＦＦ　-”あああ　いいい”"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![LayeredQuery::Query(Query::new(
                    " ＡＡＡ  \"１１１　ＣＣＣ\"  -ＤＤＤ or エエエ and ＦＦＦ  -\"あああ　いいい\" "
                        .into()
                ))])
            )
        }

        #[test]
        fn test_parse_with_bracket() {
            let query = Query::new(
                "　ＡＡＡ　”１１１　ＣＣＣ”　（-ＤＤＤ　or　エエエ）　and　ＦＦＦ　-”あああ　いいい”"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::Query(Query::new(" ＡＡＡ  \"１１１　ＣＣＣ\"  ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                        "-ＤＤＤ or エエエ".into()
                    ))])),
                    LayeredQuery::Query(Query::new(" and ＦＦＦ  -\"あああ　いいい\" ".into()))
                ])
            )
        }

        #[test]
        fn test_parse_with_negative_bracket() {
            let query = Query::new(
                "　ＡＡＡ　”１１１　ＣＣＣ”　-（ＤＤＤ　or　エエエ）　and　ＦＦＦ　-”あああ　いいい”"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::Query(Query::new(" ＡＡＡ  \"１１１　ＣＣＣ\"  ".into())),
                    LayeredQuery::NegativeBracket(LayeredQueries(vec![LayeredQuery::Query(
                        Query::new("ＤＤＤ or エエエ".into())
                    )])),
                    LayeredQuery::Query(Query::new(" and ＦＦＦ  -\"あああ　いいい\" ".into()))
                ])
            )
        }

        #[test]
        fn test_parse_with_multi_brackets() {
            let query = Query::new(
                "（　ＡＡＡ　”１１１　ＣＣＣ”）　（-ＤＤＤ　or　エエエ）　and　（ＦＦＦ　-”あああ　いいい”）"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                        " ＡＡＡ  \"１１１　ＣＣＣ\" ".into()
                    ))])),
                    LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                        "-ＤＤＤ or エエエ".into()
                    ))])),
                    LayeredQuery::Query(Query::new(" and ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                        "ＦＦＦ  -\"あああ　いいい\" ".into()
                    ))]))
                ])
            )
        }

        #[test]
        fn test_parse_with_multi_brackets_or_negative_brackets() {
            let query = Query::new(
                "（　ＡＡＡ　”１１１　ＣＣＣ”）-（ＤＤＤ　or　エエエ）　and　（ＦＦＦ　-”あああ　いいい”）"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                        " ＡＡＡ  \"１１１　ＣＣＣ\" ".into()
                    ))])),
                    LayeredQuery::NegativeBracket(LayeredQueries(vec![LayeredQuery::Query(
                        Query::new("ＤＤＤ or エエエ".into())
                    )])),
                    LayeredQuery::Query(Query::new(" and ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                        "ＦＦＦ  -\"あああ　いいい\" ".into()
                    ))]))
                ])
            )
        }

        #[test]
        fn test_parse_with_multi_nested_brackets() {
            let query = Query::new(
                "　ＡＡＡ　（”１１１　ＣＣＣ”　or　（（エエエ　or　ＦＦＦ　-”あああ　いいい”）　and　-ＤＤＤ））　and　ＥＥＥ"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::Query(Query::new(" ＡＡＡ ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![
                        LayeredQuery::Query(Query::new(" \"１１１　ＣＣＣ\"  or ".into())),
                        LayeredQuery::Bracket(LayeredQueries(vec![
                            LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(
                                Query::new("エエエ or ＦＦＦ  -\"あああ　いいい\" ".into())
                            )])),
                            LayeredQuery::Query(Query::new(" and -ＤＤＤ".into()))
                        ]))
                    ])),
                    LayeredQuery::Query(Query::new(" and ＥＥＥ".into())),
                ])
            )
        }

        #[test]
        fn test_parse_with_multi_nested_brackets_or_nested_negative_brackets() {
            let query = Query::new(
                "　ＡＡＡ　-（”１１１　ＣＣＣ”　or　（-（エエエ　or　ＦＦＦ　-”あああ　いいい”）　and　（-ＤＤＤ or -ＥＥＥ）））　and　ＦＦＦ"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::Query(Query::new(" ＡＡＡ ".into())),
                    LayeredQuery::NegativeBracket(LayeredQueries(vec![
                        LayeredQuery::Query(Query::new(" \"１１１　ＣＣＣ\"  or ".into())),
                        LayeredQuery::Bracket(LayeredQueries(vec![
                            LayeredQuery::NegativeBracket(LayeredQueries(vec![
                                LayeredQuery::Query(Query::new(
                                    "エエエ or ＦＦＦ  -\"あああ　いいい\" ".into()
                                ))
                            ])),
                            LayeredQuery::Query(Query::new(" and ".into())),
                            LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(
                                Query::new("-ＤＤＤ or -ＥＥＥ".into())
                            )]))
                        ]))
                    ])),
                    LayeredQuery::Query(Query::new(" and ＦＦＦ".into())),
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
                    LayeredQuery::Query(Query::new(" ＡＡＡ ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![
                        LayeredQuery::Query(Query::new(" \"１１１　ＣＣＣ\"  ".into())),
                        LayeredQuery::Bracket(LayeredQueries(vec![
                            LayeredQuery::NegativeBracket(LayeredQueries(vec![
                                LayeredQuery::Query(Query::new(" ＤＤＤ エエエ ".into())),
                            ])),
                            LayeredQuery::Query(Query::new(" ＦＦＦ".into())),
                        ])),
                        LayeredQuery::Query(Query::new(" ＧＧＧ ".into())),
                        LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(
                            Query::new("ＨＨＨ  -\"あああ　いいい\"  ううう".into())
                        ),]))
                    ])),
                    LayeredQuery::Query(Query::new("  \"　ＪＪＪ　\"  ".into())),
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
        fn test_parse_with_empty_bracket() {
            let query = Query::new(
                "　ＡＡＡ　（　　）　”１１１　ＣＣＣ”　（-ＤＤＤ　or　エエエ）　and　ＦＦＦ　（）　-”あああ　いいい”"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::Query(Query::new(" ＡＡＡ   \"１１１　ＣＣＣ\"  ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                        "-ＤＤＤ or エエエ".into()
                    ))])),
                    LayeredQuery::Query(Query::new(" and ＦＦＦ   -\"あああ　いいい\" ".into()))
                ])
            )
        }

        #[test]
        fn test_parse_with_only_start_bracket() {
            let query = Query::new(
                "　ＡＡＡ　”１１１　ＣＣＣ”　-（ＤＤＤ　or　エエエ　and　（　ＦＦＦ　-”あああ　いいい”"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![LayeredQuery::Query(Query::new(" ＡＡＡ  \"１１１　ＣＣＣ\"  -ＤＤＤ or エエエ and  ＦＦＦ  -\"あああ　いいい\" "
                    .into()))])
            )
        }

        #[test]
        fn test_parse_with_only_end_bracket() {
            let query = Query::new(
                "　ＡＡＡ　”１１１　ＣＣＣ”）　-ＤＤＤ　or　エエエ　）　and　ＦＦＦ　-”あああ　いいい”"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![LayeredQuery::Query(Query::new(" ＡＡＡ  \"１１１　ＣＣＣ\"  -ＤＤＤ or エエエ  and ＦＦＦ  -\"あああ　いいい\" "
                    .into()))])
            )
        }

        #[test]
        fn test_parse_with_start_bracket_more_than_end_bracket() {
            let query = Query::new(
                "　ＡＡＡ　（”１１１　ＣＣＣ”　（ＤＤＤ　or　エエエ）　and　（　ＦＦＦ　-”あああ　いいい”"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::Query(Query::new(" ＡＡＡ  \"１１１　ＣＣＣ\"  ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                        "ＤＤＤ or エエエ".into()
                    ))])),
                    LayeredQuery::Query(Query::new(" and  ＦＦＦ  -\"あああ　いいい\" ".into()))
                ])
            )
        }

        #[test]
        fn test_parse_with_start_bracket_less_than_end_bracket() {
            let query = Query::new(
                "　ＡＡＡ）　”１１１　ＣＣＣ”　（ＤＤＤ　or　エエエ）　and　）　ＦＦＦ　-”あああ　いいい”"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::Query(Query::new(" ＡＡＡ  \"１１１　ＣＣＣ\"  ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                        "ＤＤＤ or エエエ".into()
                    ))])),
                    LayeredQuery::Query(Query::new(" and  ＦＦＦ  -\"あああ　いいい\" ".into()))
                ])
            )
        }

        #[test]
        fn test_parse_with_reverse_bracket() {
            let query = Query::new(
                "　ＡＡＡ）　”１１１　ＣＣＣ”　（ＤＤＤ　or　エエエ）　and　（　ＦＦＦ　-”あああ　いいい”"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::Query(Query::new(" ＡＡＡ  \"１１１　ＣＣＣ\"  ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                        "ＤＤＤ or エエエ".into()
                    ))])),
                    LayeredQuery::Query(Query::new(" and  ＦＦＦ  -\"あああ　いいい\" ".into()))
                ])
            )
        }

        #[test]
        fn test_parse_bracket_in_phrase_keywords() {
            let query = Query::new(
                "-（Ａ１　or　\" Ｐ１ and Ｐ２ -(Ｐ３ or Ｐ４) \"）　and　（-\" ＮＰ１ and ＮＰ２ -(ＮＰ３ or ＮＰ４) \"　or　Ａ２）"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::NegativeBracket(LayeredQueries(vec![LayeredQuery::Query(
                        Query::new("Ａ１ or  \" Ｐ１ and Ｐ２ -(Ｐ３ or Ｐ４) \" ".into())
                    )])),
                    LayeredQuery::Query(Query::new(" and ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                        " -\" ＮＰ１ and ＮＰ２ -(ＮＰ３ or ＮＰ４) \"  or Ａ２".into()
                    ))]))
                ])
            )
        }

        #[test]
        fn test_parse_full_width_bracket_in_phrase_keywords() {
            let query = Query::new(
                "-（Ａ１　or　”　Ｐ１　ａｎｄ　Ｐ２　−（Ｐ３　ｏｒ　Ｐ４）　”）　and　（-”　ＮＰ１　ａｎｄ　ＮＰ２　−（ＮＰ３　ｏｒ　ＮＰ４）　”　or　Ａ２）"
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query).unwrap(),
                LayeredQueries(vec![
                    LayeredQuery::NegativeBracket(LayeredQueries(vec![LayeredQuery::Query(
                        Query::new(
                            "Ａ１ or  \"　Ｐ１　ａｎｄ　Ｐ２　−（Ｐ３　ｏｒ　Ｐ４）　\" ".into()
                        )
                    )])),
                    LayeredQuery::Query(Query::new(" and ".into())),
                    LayeredQuery::Bracket(LayeredQueries(vec![LayeredQuery::Query(Query::new(
                        " -\"　ＮＰ１　ａｎｄ　ＮＰ２　−（ＮＰ３　ｏｒ　ＮＰ４）　\"  or Ａ２"
                            .into()
                    ))]))
                ])
            )
        }
    }

    mod test_layered_queries_parse_to_condition {
        use super::*;

        #[test]
        fn test_layered_queries_parse_to_condition_empty_string() {
            let query = Query::new("".into());
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::None
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_blank_string() {
            let query = Query::new(" 　 ".into());
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::None
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_one_keyword() {
            let query = Query::new(" 検索 ".into());
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::Keyword("検索".into())
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_one_phrase_keyword() {
            let query = Query::new(" \"検索\" ".into());
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::PhraseKeyword("検索".into())
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_one_negative_keyword() {
            let query = Query::new(" -検索 ".into());
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::Not(Box::new(Condition::Keyword("検索".into())))
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_one_negative_phrase_keyword() {
            let query = Query::new(" -\"検索\" ".into());
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::Not(Box::new(Condition::PhraseKeyword("検索".into())))
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_mutlti_keywords_concat_with_space() {
            let query = Query::new(" 検索１ -検索２ \"検索３\" -\"検索４\" ".into());
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("検索１".into()),
                        Condition::Not(Box::new(Condition::Keyword("検索２".into()))),
                        Condition::PhraseKeyword("検索３".into()),
                        Condition::Not(Box::new(Condition::PhraseKeyword("検索４".into())))
                    ]
                )
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_mutlti_keywords_concat_with_and() {
            let query = Query::new(" 検索１ and -検索２ and \"検索３\" and -\"検索４\" ".into());
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("検索１".into()),
                        Condition::Not(Box::new(Condition::Keyword("検索２".into()))),
                        Condition::PhraseKeyword("検索３".into()),
                        Condition::Not(Box::new(Condition::PhraseKeyword("検索４".into())))
                    ]
                )
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_mutlti_keywords_concat_with_or() {
            let query = Query::new(" 検索１ or -検索２ or \"検索３\" or -\"検索４\" ".into());
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("検索１".into()),
                        Condition::Not(Box::new(Condition::Keyword("検索２".into()))),
                        Condition::PhraseKeyword("検索３".into()),
                        Condition::Not(Box::new(Condition::PhraseKeyword("検索４".into())))
                    ]
                )
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_mutlti_keywords_concat_with_space_or_and() {
            let query = Query::new(" 検索１ -検索２ or \"検索３\" and -\"検索４\" ".into());
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("検索１".into()),
                                Condition::Not(Box::new(Condition::Keyword("検索２".into()))),
                            ]
                        ),
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::PhraseKeyword("検索３".into()),
                                Condition::Not(Box::new(Condition::PhraseKeyword("検索４".into())))
                            ]
                        )
                    ]
                )
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_or_in_brackets() {
            let query = Query::new(" 検索１ and (-検索２ or \"検索３\") or -\"検索４\" ".into());
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("検索１".into()),
                                Condition::Operator(
                                    Operator::Or,
                                    vec![
                                        Condition::Not(Box::new(Condition::Keyword(
                                            "検索２".into()
                                        ))),
                                        Condition::PhraseKeyword("検索３".into()),
                                    ]
                                ),
                            ]
                        ),
                        Condition::Not(Box::new(Condition::PhraseKeyword("検索４".into())))
                    ]
                )
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_or_in_negative_brackets() {
            let query = Query::new(" 検索１ and -(-検索２ or \"検索３\") or -\"検索４\" ".into());
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("検索１".into()),
                                Condition::Not(Box::new(Condition::Operator(
                                    Operator::Or,
                                    vec![
                                        Condition::Not(Box::new(Condition::Keyword(
                                            "検索２".into()
                                        ))),
                                        Condition::PhraseKeyword("検索３".into()),
                                    ]
                                ))),
                            ]
                        ),
                        Condition::Not(Box::new(Condition::PhraseKeyword("検索４".into())))
                    ]
                )
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_mutlti_brackets() {
            let query = Query::new(
                " (検索１ or -検索２)and(\"検索３\" or -\"検索４\")(\" 検索５ 検索６ \" or 検索７) "
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::Keyword("検索１".into()),
                                Condition::Not(Box::new(Condition::Keyword("検索２".into()))),
                            ]
                        ),
                        Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::PhraseKeyword("検索３".into()),
                                Condition::Not(Box::new(Condition::PhraseKeyword("検索４".into())))
                            ]
                        ),
                        Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::PhraseKeyword(" 検索５ 検索６ ".into()),
                                Condition::Keyword("検索７".into())
                            ]
                        )
                    ]
                )
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_nested_brackets() {
            let query = Query::new(
                " (word１ and -word２) or ((\"phrase word １\" or -\"phrase word ２\") and -(\" a long phrase word \" or word３)) "
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("word１".into()),
                                Condition::Not(Box::new(Condition::Keyword("word２".into()))),
                            ]
                        ),
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Operator(
                                    Operator::Or,
                                    vec![
                                        Condition::PhraseKeyword("phrase word １".into()),
                                        Condition::Not(Box::new(Condition::PhraseKeyword(
                                            "phrase word ２".into()
                                        )))
                                    ]
                                ),
                                Condition::Not(Box::new(Condition::Operator(
                                    Operator::Or,
                                    vec![
                                        Condition::PhraseKeyword(" a long phrase word ".into()),
                                        Condition::Keyword("word３".into())
                                    ]
                                )))
                            ]
                        ),
                    ]
                )
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_bracket_in_phrase_keywords() {
            let query = Query::new(
                " (検索１ and -検索２) or ((\" Ｐ１ and Ｐ２ -(Ｐ３ or Ｐ４) \" or -\" ＮＰ１ and ＮＰ２ -(ＮＰ３ or ＮＰ４) \") and (\" 検索５ 検索６ \" or 検索７)) "
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("検索１".into()),
                                Condition::Not(Box::new(Condition::Keyword("検索２".into()))),
                            ]
                        ),
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Operator(
                                    Operator::Or,
                                    vec![
                                        Condition::PhraseKeyword(
                                            " Ｐ１ and Ｐ２ -(Ｐ３ or Ｐ４) ".into()
                                        ),
                                        Condition::Not(Box::new(Condition::PhraseKeyword(
                                            " ＮＰ１ and ＮＰ２ -(ＮＰ３ or ＮＰ４) ".into()
                                        )))
                                    ]
                                ),
                                Condition::Operator(
                                    Operator::Or,
                                    vec![
                                        Condition::PhraseKeyword(" 検索５ 検索６ ".into()),
                                        Condition::Keyword("検索７".into())
                                    ]
                                )
                            ]
                        ),
                    ]
                )
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_unnecessary_nested_brackets() {
            let query = Query::new(
                " ((検索１ and -検索２)) or (((((\"検索３\" or -\"検索４\"))) and ((((\" 検索５ 検索６ \" or 検索７)))))) "
                    .into(),
            );
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("検索１".into()),
                                Condition::Not(Box::new(Condition::Keyword("検索２".into()))),
                            ]
                        ),
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Operator(
                                    Operator::Or,
                                    vec![
                                        Condition::PhraseKeyword("検索３".into()),
                                        Condition::Not(Box::new(Condition::PhraseKeyword(
                                            "検索４".into()
                                        )))
                                    ]
                                ),
                                Condition::Operator(
                                    Operator::Or,
                                    vec![
                                        Condition::PhraseKeyword(" 検索５ 検索６ ".into()),
                                        Condition::Keyword("検索７".into())
                                    ]
                                )
                            ]
                        ),
                    ]
                )
            )
        }

        #[test]
        fn test_layered_queries_parse_to_condition_full_pattern() {
            let query =
                Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　or（-（　ＤＤＤ　or　エエエ　）and　ＦＦＦ）or　ＧＧＧ　（ＨＨＨ　or　-”あああ　いいい”　ううう））　”　ＪＪＪ　”　or　-（ＫＫＫ　and　（　）　or　ＬＬＬ）　　（ＭＭＭ）or　２２２　".into());
            assert_eq!(
                LayeredQueries::parse(query)
                    .unwrap()
                    .to_condition()
                    .unwrap(),
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("ＡＡＡ".into()),
                                Condition::Operator(
                                    Operator::Or,
                                    vec![
                                        Condition::PhraseKeyword("１１１　ＣＣＣ".into()),
                                        Condition::Operator(
                                            Operator::And,
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
                                        Condition::Operator(
                                            Operator::And,
                                            vec![
                                                Condition::Keyword("ＧＧＧ".into()),
                                                Condition::Operator(
                                                    Operator::Or,
                                                    vec![
                                                        Condition::Keyword("ＨＨＨ".into()),
                                                        Condition::Operator(
                                                            Operator::And,
                                                            vec![
                                                                Condition::Not(Box::new(
                                                                    Condition::PhraseKeyword(
                                                                        "あああ　いいい".into()
                                                                    )
                                                                )),
                                                                Condition::Keyword("ううう".into()),
                                                            ]
                                                        )
                                                    ]
                                                )
                                            ]
                                        )
                                    ]
                                ),
                                Condition::PhraseKeyword("　ＪＪＪ　".into())
                            ]
                        ),
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Not(Box::new(Condition::Operator(
                                    Operator::Or,
                                    vec![
                                        Condition::Keyword("ＫＫＫ".into()),
                                        Condition::Keyword("ＬＬＬ".into()),
                                    ]
                                ))),
                                Condition::Keyword("ＭＭＭ".into()),
                            ]
                        ),
                        Condition::Keyword("２２２".into())
                    ]
                )
            )
        }
    }
}
