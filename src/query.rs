use crate::condition::Condition;
use crate::layered_query::{LayeredQueries, LayeredQuery};
use crate::Operator;
use eyre::Result;
use regex::{Captures, Match, Regex};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Query(String);

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

    fn filter_not_blank_query(regex_match: Option<Match>) -> Option<Query> {
        regex_match
            .map(|m| Query::new(m.as_str().into()))
            .filter(|q| q.is_not_blank())
    }

    fn match_to_number<F: FnOnce(usize) -> Option<R>, R>(
        regex_match: Option<Match>, call_back: F,
    ) -> Option<R> {
        regex_match
            .map(|m| m.as_str().parse::<usize>())
            .map(|index| index.map(|i| call_back(i)).unwrap_or(None))
            .flatten()
    }

    pub fn value(&self) -> &str {
        self.0.as_str()
    }

    fn is_not_blank(&self) -> bool {
        self.value().replace(" ", "").is_empty() == false
    }

    pub fn layered_by_bracket(self) -> Result<LayeredQueries> {
        fn pick_layer_by_bracket(
            query: String, bracket_queries: &mut Vec<Query>,
        ) -> Result<String> {
            let regex_bracket = Regex::new(r"\(([^\(\)]*)\)")?;
            let innermost_bracket_removed_query = regex_bracket
                .replace_all(query.as_str(), |captures: &Captures| {
                    match Query::filter_not_blank_query(captures.get(1)) {
                        Some(q) => {
                            bracket_queries.push(q);
                            format!("（{}）", bracket_queries.len())
                        }
                        None => String::from(""),
                    }
                })
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
                    Query::filter_not_blank_query(captures.get(1)).map(|mut q| {
                        if q.value().ends_with("-") {
                            is_negative_bracket = true;
                            q = Query::new(String::from(&q.value()[0..q.value().len() - 1]))
                        }
                        if q.is_not_blank() {
                            layered_queries.push(LayeredQuery::Query(q))
                        }
                    });
                    Query::match_to_number(captures.get(2), |i| {
                        bracket_queries.get(i - 1).map(|q: &Query| {
                            combine_layered_query(q.clone(), bracket_queries).map(|v| {
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
            Ok(LayeredQueries::new(layered_queries))
        }

        Ok(combine_layered_query(
            Query::new(all_brackets_replace_query),
            &bracket_queries,
        )?)
    }

    pub(crate) fn parse_to_condition(self) -> Result<Condition> {
        let mut query = self.0;
        let mut negative_exact_keywords = Vec::<Query>::new();
        let mut exact_keywords = Vec::<Query>::new();
        vec![
            (
                Regex::new("-\"([^\"]*)\"")?,
                &mut negative_exact_keywords,
                "NEK",
            ),
            (Regex::new("\"([^\"]*)\"")?, &mut exact_keywords, "EK"),
        ]
        .iter_mut()
        .for_each(|(regex, vec, prefix)| {
            query = regex
                .replace_all(query.as_str(), |captures: &Captures| {
                    match Query::filter_not_blank_query(captures.get(1)) {
                        Some(q) => {
                            vec.push(q);
                            format!("({}:{})", prefix, vec.len())
                        }
                        None => String::from(""),
                    }
                })
                .to_string()
        });

        query = Regex::new(" +(?i)[A|Ａ](?i)[N|Ｎ](?i)[D|Ｄ] +")?
            .replace_all(query.as_str(), |_: &Captures| String::from(" "))
            .to_string();

        let mut or_conditions = Vec::<Condition>::new();
        let (mut is_start_with_or, mut is_end_with_or) = (false, false);
        let or_queries = Regex::new(" +(?i)[O|Ｏ](?i)[R|Ｒ] +")?
            .split(query.as_str())
            .into_iter()
            .collect::<Vec<&str>>();
        let and_regex = Regex::new(" +")?;
        let or_queries_last_index = or_queries.len() - 1;
        or_queries.into_iter().enumerate().for_each(|(i, q)| {
            let query = Query::new(q.into());
            match (query.is_not_blank(), i) {
                (false, index) => {
                    if index == 0 {
                        is_start_with_or = true
                    } else if index == or_queries_last_index {
                        is_end_with_or = true
                    }
                }
                (true, _) => {
                    let and_conditions = and_regex
                        .split(query.value())
                        .into_iter()
                        .filter_map(|k| {
                            let q = Query::new(k.into());
                            match q.is_not_blank() {
                                true => Some(q),
                                false => None,
                            }
                        })
                        .filter_map(|keyword| {
                            keyword_condition(
                                keyword.value(),
                                &negative_exact_keywords,
                                &exact_keywords,
                            )
                            .unwrap_or(None)
                        })
                        .collect::<Vec<Condition>>();
                    or_conditions.push(Condition::Operator(Operator::And, and_conditions));
                }
            }
        });

        return Ok(Condition::Operator(
            Operator::And,
            vec![Condition::Operator(Operator::Or, or_conditions)],
        ));

        fn keyword_condition(
            k: &str, negative_exact_keywords: &Vec<Query>, exact_keywords: &Vec<Query>,
        ) -> Result<Option<Condition>> {
            Ok(
                match (
                    Regex::new(r"^\(NEK:(\d)\)$")?.captures(k),
                    Regex::new(r"^\(EK:(\d)\)$")?.captures(k),
                ) {
                    (Some(nek), _) => Query::match_to_number(nek.get(1), |i| {
                        negative_exact_keywords.get(i - 1).map(|nek| {
                            Condition::Negative(Box::new(Condition::ExactKeyword(
                                nek.value().to_string(),
                            )))
                        })
                    }),
                    (_, Some(ek)) => Query::match_to_number(ek.get(1), |i| {
                        exact_keywords
                            .get(i - 1)
                            .map(|ek| Condition::ExactKeyword(ek.value().to_string()))
                    }),
                    (None, None) => match (k.len(), k.starts_with("-")) {
                        (1, _) => Some(Condition::Keyword(k.into())),
                        (_, true) => Some(Condition::Negative(Box::new(Condition::Keyword(
                            k[1..k.len()].into(),
                        )))),
                        _ => Some(Condition::Keyword(k.into())),
                    },
                },
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layered_query::LayeredQueries;

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
            LayeredQueries::new(vec![
                LayeredQuery::Query(Query::new(" ＡＡＡ ".into())),
                LayeredQuery::Bracket(LayeredQueries::new(vec![
                    LayeredQuery::Query(Query::new("\"１１１ ＣＣＣ\" ".into())),
                    LayeredQuery::Bracket(LayeredQueries::new(vec![
                        LayeredQuery::NegativeBracket(LayeredQueries::new(vec![
                            LayeredQuery::Query(Query::new(" ＤＤＤ エエエ ".into())),
                        ])),
                        LayeredQuery::Query(Query::new(" ＦＦＦ".into())),
                    ])),
                    LayeredQuery::Query(Query::new(" ＧＧＧ ".into())),
                    LayeredQuery::Bracket(LayeredQueries::new(vec![LayeredQuery::Query(
                        Query::new("ＨＨＨ -\"あああ いいい\" ううう".into())
                    ),]))
                ])),
                LayeredQuery::Query(Query::new(" \" ＪＪＪ \" ".into())),
                LayeredQuery::NegativeBracket(LayeredQueries::new(vec![LayeredQuery::Query(
                    Query::new("ＫＫＫ  ＬＬＬ".into())
                ),])),
                LayeredQuery::Bracket(LayeredQueries::new(vec![LayeredQuery::Query(Query::new(
                    "ＭＭＭ".into()
                )),])),
                LayeredQuery::Query(Query::new(" ２２２ ".into())),
            ])
        )
    }

    #[test]
    fn test_query_parse_to_condition_only_space() {
        let target = Query::new("　   　".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(actual, Condition::Operator(Operator::And, vec![]))
    }

    #[test]
    fn test_query_parse_to_condition_only_one_keyword() {
        let target = Query::new("ＡＡＡ".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(Operator::And, vec![Condition::Keyword("ＡＡＡ".into())])
        )
    }

    #[test]
    fn test_query_parse_to_condition_only_one_exact_keyword() {
        let target = Query::new("\"ＡＡＡ　ＢＢＢ\"".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::And,
                vec![Condition::ExactKeyword("ＡＡＡ ＢＢＢ".into())]
            )
        )
    }

    #[test]
    fn test_query_parse_to_condition_only_one_negative_keyword() {
        let target = Query::new("-ＡＡＡ".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::And,
                vec![Condition::Negative(Box::new(Condition::Keyword(
                    "ＡＡＡ".into()
                )))]
            )
        )
    }

    #[test]
    fn test_query_parse_to_condition_only_one_negative_exact_keyword() {
        let target = Query::new("-\"ＡＡＡ　ＢＢＢ\"".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::And,
                vec![Condition::Negative(Box::new(Condition::ExactKeyword(
                    "ＡＡＡ ＢＢＢ".into()
                )))]
            )
        )
    }

    #[test]
    fn test_query_parse_to_condition_two_keywords() {
        let target = Query::new("ＡＡＡ　ＢＢＢ".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::And,
                vec![
                    Condition::Keyword("ＡＡＡ".into()),
                    Condition::Keyword("ＢＢＢ".into())
                ]
            )
        )
    }

    #[test]
    fn test_query_parse_to_condition_two_exact_keywords() {
        let target = Query::new("\"ＡＡＡ　ＢＢＢ\"　\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::And,
                vec![
                    Condition::ExactKeyword("ＡＡＡ ＢＢＢ".into()),
                    Condition::ExactKeyword("ＣＣＣ ＤＤＤ".into())
                ]
            )
        )
    }

    #[test]
    fn test_query_parse_to_condition_two_negative_keywords() {
        let target = Query::new("-ＡＡＡ　-ＢＢＢ".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::And,
                vec![
                    Condition::Negative(Box::new(Condition::Keyword("ＡＡＡ".into()))),
                    Condition::Negative(Box::new(Condition::Keyword("ＢＢＢ".into())))
                ]
            )
        )
    }

    #[test]
    fn test_query_parse_to_condition_two_negative_exact_keywords() {
        let target = Query::new("-\"ＡＡＡ　ＢＢＢ\"　-\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::And,
                vec![
                    Condition::Negative(Box::new(Condition::ExactKeyword("ＡＡＡ ＢＢＢ".into()))),
                    Condition::Negative(Box::new(Condition::ExactKeyword("ＣＣＣ ＤＤＤ".into())))
                ]
            )
        )
    }

    #[test]
    fn test_query_parse_to_condition_two_keywords_with_or() {
        let target = Query::new("ＡＡＡ or　ＢＢＢ".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::Or,
                vec![
                    Condition::Keyword("ＡＡＡ".into()),
                    Condition::Keyword("ＢＢＢ".into())
                ]
            )
        )
    }

    #[test]
    fn test_query_parse_to_condition_two_exact_keywords_with_or() {
        let target = Query::new("\"ＡＡＡ　ＢＢＢ\" or　\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::Or,
                vec![
                    Condition::ExactKeyword("ＡＡＡ ＢＢＢ".into()),
                    Condition::ExactKeyword("ＣＣＣ ＤＤＤ".into())
                ]
            )
        )
    }

    #[test]
    fn test_query_parse_to_condition_two_negative_keywords_with_or() {
        let target = Query::new("-ＡＡＡ or　-ＢＢＢ".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::Or,
                vec![
                    Condition::Negative(Box::new(Condition::Keyword("ＡＡＡ".into()))),
                    Condition::Negative(Box::new(Condition::Keyword("ＢＢＢ".into())))
                ]
            )
        )
    }

    #[test]
    fn test_query_parse_to_condition_two_negative_exact_keywords_with_or() {
        let target = Query::new("-\"ＡＡＡ　ＢＢＢ\" or　-\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::Or,
                vec![
                    Condition::Negative(Box::new(Condition::ExactKeyword("ＡＡＡ ＢＢＢ".into()))),
                    Condition::Negative(Box::new(Condition::ExactKeyword("ＣＣＣ ＤＤＤ".into())))
                ]
            )
        )
    }

    #[test]
    fn test_query_parse_to_condition_two_keywords_with_and() {
        let target = Query::new("ＡＡＡ and　ＢＢＢ".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::And,
                vec![
                    Condition::Keyword("ＡＡＡ".into()),
                    Condition::Keyword("ＢＢＢ".into())
                ]
            )
        )
    }

    #[test]
    fn test_query_parse_to_condition_two_exact_keywords_with_and() {
        let target = Query::new("\"ＡＡＡ　ＢＢＢ\" and　\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::And,
                vec![
                    Condition::ExactKeyword("ＡＡＡ ＢＢＢ".into()),
                    Condition::ExactKeyword("ＣＣＣ ＤＤＤ".into())
                ]
            )
        )
    }

    #[test]
    fn test_query_parse_to_condition_two_negative_keywords_with_and() {
        let target = Query::new("-ＡＡＡ and　-ＢＢＢ".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::And,
                vec![
                    Condition::Negative(Box::new(Condition::Keyword("ＡＡＡ".into()))),
                    Condition::Negative(Box::new(Condition::Keyword("ＢＢＢ".into())))
                ]
            )
        )
    }

    #[test]
    fn test_query_parse_to_condition_two_negative_exact_keywords_with_and() {
        let target = Query::new("-\"ＡＡＡ　ＢＢＢ\" and　-\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::And,
                vec![
                    Condition::Negative(Box::new(Condition::ExactKeyword("ＡＡＡ ＢＢＢ".into()))),
                    Condition::Negative(Box::new(Condition::ExactKeyword("ＣＣＣ ＤＤＤ".into())))
                ]
            )
        )
    }

    #[test]
    fn test_query_parse_to_condition_multi_keywords_with_or_and() {}

    #[test]
    fn test_query_parse_to_condition_full_pattern() {
        let target = Query::new("　ＡＡＡ　　Ａｎｄ　-ＢＢＢ　ＡnＤ　ＣorＣ　　ｃｃｃ　Ｏr　　\"c1 and c2\"　　-\"c3 or c4\"　　ＤandＤ　anD　\"　ＥＥＥ　ＡNＤ　ＦＦＦ　\"　　ａnｄ　　-\"　ＧＧＧ　　oＲ　　ＨＨＨ　\"　　oＲ　　ＩＩＩ　and　".into());
        let actual = target.parse_to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::Or,
                vec![
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Keyword("ＡＡＡ".into()),
                            Condition::Negative(Box::new(Condition::Keyword("ＢＢＢ".into()))),
                            Condition::Keyword("ＣorＣ".into()),
                        ]
                    ),
                    Condition::Keyword("ｃｃｃ".into()),
                    Condition::ExactKeyword("c1 and c2".into()),
                    Condition::Negative(Box::new(Condition::ExactKeyword("c3 or c4".into()))),
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Keyword("ＤandＤ".into()),
                            Condition::ExactKeyword(" ＥＥＥ ＡNＤ ＦＦＦ ".into()),
                            Condition::Negative(Box::new(Condition::ExactKeyword(
                                " ＧＧＧ  oＲ  ＨＨＨ ".into()
                            )))
                        ]
                    ),
                    Condition::Operator(Operator::And, vec![Condition::Keyword("ＩＩＩ".into())]),
                ]
            )
        )
    }
}
