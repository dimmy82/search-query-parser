use crate::{Condition, LayeredQueries, LayeredQuery, Operator};
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

    fn parse_to_condition(self) -> Result<Condition> {
        let mut negative_exact_keywords = Vec::<Query>::new();
        let mut exact_keywords = Vec::<Query>::new();

        let mut query = self.0;
        vec![
            (
                Regex::new("-\"(.*)\"")?,
                &mut negative_exact_keywords,
                "NEK",
            ),
            (Regex::new("\"(.*)\"")?, &mut exact_keywords, "EK"),
        ]
        .iter_mut()
        .for_each(|(regex, vec, prefix)| {
            query = regex
                .replace_all(query.as_str(), |captures: &Captures| {
                    match Query::filter_not_blank_query(captures.get(1)) {
                        Some(q) => {
                            vec.push(q);
                            format!("（{}:{}）", prefix, vec.len())
                        }
                        None => String::from(""),
                    }
                })
                .to_string()
        });

        query = Regex::new(" +(?i)[O|Ｏ](?i)[R|Ｒ] +")?
            .replace_all(query.as_str(), |_: &Captures| String::from(" "))
            .to_string();

        let keywords = Regex::new(" +")?
            .split(query.as_str())
            .into_iter()
            .filter(|k| !k.is_empty())
            .collect::<Vec<&str>>();
        println!("{:?}", keywords);

        let mut current_operator = Operator::Or;
        let mut current_conditions = Vec::<Condition>::new();
        let mut is_last_keyword_a_operator = false;
        for keyword in keywords.into_iter() {
            match keyword {
                k if Regex::new("^(?i)[A|Ａ](?i)[N|Ｎ](?i)[D|Ｄ]$")?.is_match(k) => {}
                k => {
                    let condition = match (
                        Regex::new(r"^（NEK:(\d)）$")?.captures(k),
                        Regex::new(r"^（EK:(\d)）$")?.captures(k),
                    ) {
                        (Some(nek), _) => Query::match_to_number(nek.get(1), |i| {
                            negative_exact_keywords.get(i).map(|nek| {
                                Condition::Negative(Box::new(Condition::ExactKeyword(
                                    nek.value().to_string(),
                                )))
                            })
                        }),
                        (_, Some(ek)) => Query::match_to_number(ek.get(1), |i| {
                            exact_keywords
                                .get(i)
                                .map(|ek| Condition::ExactKeyword(ek.value().to_string()))
                        }),
                        (None, None) => Some(if k.starts_with("-") {
                            Condition::Negative(Box::new(Condition::Keyword(k.into())))
                        } else {
                            Condition::Keyword(k.into())
                        }),
                    };
                    condition.map(|c| {
                        match (is_last_keyword_a_operator, &current_operator) {
                            (false, &Operator::And) => {
                                let mut and_conditions = Vec::<Condition>::new();
                                and_conditions.append(&mut current_conditions);
                                current_conditions = Vec::<Condition>::new();
                                current_conditions
                                    .push(Condition::Operator(Operator::And, and_conditions));
                                current_conditions.push(c);
                                current_operator = Operator::Or;
                            }
                            _ => current_conditions.push(c),
                        }
                        is_last_keyword_a_operator = false;
                    });
                }
            }
        }

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
    fn test_query_parse_to_condition() {
        let target = Query::new("　ＡＡＡ　　Ａｎｄ　-ＢＢＢ　ＡnＤ　ＣorＣ　　ｃｃｃ　Ｏr　　ＤandＤ　anD　\"　ＥＥＥ　ＡNＤ　ＦＦＦ　\"　　ａnｄ　　-\"　ＧＧＧ　　oＲ　　ＨＨＨ　\"　　oＲ　　ＩＩＩ　and　".into());
        assert_eq!(
            target.parse_to_condition().unwrap(),
            Condition::Operator(
                Operator::Or,
                vec![
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Keyword("ＡＡＡ".into()),
                            Condition::Negative(Box::new(Condition::Keyword("ＢＢＢ".into())))
                        ]
                    ),
                    Condition::Keyword("ＣorＣ".into()),
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Keyword("ＤＤＤ".into()),
                            Condition::ExactKeyword("ＥＥＥ and ＦＦＦ".into()),
                            Condition::Negative(Box::new(Condition::ExactKeyword(
                                "ＧＧＧ or ＨＨＨ".into()
                            )))
                        ]
                    ),
                    Condition::Keyword("ＩＩＩ".into()),
                ]
            )
        )
    }
}
