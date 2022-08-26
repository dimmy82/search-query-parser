use crate::condition::Condition;
use crate::layered_query::{LayeredQueries, LayeredQuery};
use crate::{is_not_blank, Operator};
use eyre::Result;
use regex::{Captures, Match, Regex};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Query(String);

impl Query {
    pub(crate) fn new(value: String) -> Self {
        Query(
            value
                .replace("（", "(")
                .replace("）", ")")
                .replace("”", "\"")
                .replace("　", " "),
        )
    }

    pub(crate) fn value(self) -> String {
        self.0
    }

    pub(crate) fn value_ref(&self) -> &str {
        self.0.as_str()
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

    pub(crate) fn is_not_blank(&self) -> bool {
        is_not_blank(self.value_ref())
    }

    pub(crate) fn to_condition(self) -> Result<Condition> {
        let (mut query, negative_exact_keywords, exact_keywords) = self.extract_exact_keyword()?;

        query = Query::new(
            Regex::new(" +(?i)[A|Ａ](?i)[N|Ｎ](?i)[D|Ｄ] +")?
                .replace_all(query.value_ref(), |_: &Captures| String::from(" "))
                .to_string(),
        );

        let mut or_conditions = Vec::<Condition>::new();
        let (mut is_start_with_or, mut is_end_with_or) = (false, false);
        let or_queries = Regex::new(" +(?i)[O|Ｏ](?i)[R|Ｒ] +")?
            .split(query.value_ref())
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
                        .split(query.value_ref())
                        .into_iter()
                        .filter_map(|k| {
                            let q = Query::new(k.into());
                            match q.is_not_blank() {
                                true => Some(q),
                                false => None,
                            }
                        })
                        .filter_map(|keyword| {
                            keyword
                                .keyword_condition(&negative_exact_keywords, &exact_keywords)
                                .unwrap_or(None)
                        })
                        .collect::<Vec<Condition>>();
                    or_conditions.push(Condition::Operator(Operator::And, and_conditions));
                }
            }
        });

        return Ok(Condition::Operator(Operator::Or, or_conditions).simplify());
    }

    fn extract_exact_keyword(self) -> Result<(Self, Vec<Query>, Vec<Query>)> {
        let mut query = self;
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
            query = Query::new(
                regex
                    .replace_all(query.value_ref(), |captures: &Captures| {
                        match Query::filter_not_blank_query(captures.get(1)) {
                            Some(q) => {
                                vec.push(q);
                                format!("({}:{})", prefix, vec.len())
                            }
                            None => String::from(""),
                        }
                    })
                    .to_string(),
            )
        });
        Ok((query, negative_exact_keywords, exact_keywords))
    }

    fn keyword_condition(
        self, negative_exact_keywords: &Vec<Query>, exact_keywords: &Vec<Query>,
    ) -> Result<Option<Condition>> {
        Ok(
            match (
                Regex::new(r"^\(NEK:(\d)\)$")?.captures(self.value_ref()),
                Regex::new(r"^\(EK:(\d)\)$")?.captures(self.value_ref()),
            ) {
                (Some(nek), _) => Query::match_to_number(nek.get(1), |i| {
                    negative_exact_keywords.get(i - 1).map(|nek| {
                        Condition::Negative(Box::new(Condition::ExactKeyword(
                            nek.value_ref().to_string(),
                        )))
                    })
                }),
                (_, Some(ek)) => Query::match_to_number(ek.get(1), |i| {
                    exact_keywords
                        .get(i - 1)
                        .map(|ek| Condition::ExactKeyword(ek.value_ref().to_string()))
                }),
                (None, None) => match (self.value_ref().len(), self.value_ref().starts_with("-")) {
                    (1, _) => Some(Condition::Keyword(self.value())),
                    (_, true) => Some(Condition::Negative(Box::new(Condition::Keyword(
                        self.value_ref()[1..self.value_ref().len()].into(),
                    )))),
                    _ => Some(Condition::Keyword(self.value())),
                },
            },
        )
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
            target.value_ref(),
            " ＡＡＡ (\"１１１ ＣＣＣ\" (-( ＤＤＤ エエエ ) ＦＦＦ) ＧＧＧ (ＨＨＨ -\"あああ いいい\" ううう)) \" ＪＪＪ \" -(ＫＫＫ ( ) ＬＬＬ)  (ＭＭＭ) ２２２ "
        )
    }

    #[test]
    fn test_query_to_condition_only_space() {
        let target = Query::new("　   　".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(actual, Condition::None)
    }

    #[test]
    fn test_query_to_condition_only_one_keyword() {
        let target = Query::new("ＡＡＡ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(actual, Condition::Keyword("ＡＡＡ".into()))
    }

    #[test]
    fn test_query_to_condition_only_one_exact_keyword() {
        let target = Query::new("\"ＡＡＡ　ＢＢＢ\"".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(actual, Condition::ExactKeyword("ＡＡＡ ＢＢＢ".into()))
    }

    #[test]
    fn test_query_to_condition_only_one_negative_keyword() {
        let target = Query::new("-ＡＡＡ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Negative(Box::new(Condition::Keyword("ＡＡＡ".into())))
        )
    }

    #[test]
    fn test_query_to_condition_only_one_negative_exact_keyword() {
        let target = Query::new("-\"ＡＡＡ　ＢＢＢ\"".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Negative(Box::new(Condition::ExactKeyword("ＡＡＡ ＢＢＢ".into())))
        )
    }

    #[test]
    fn test_query_to_condition_only_one_double_negative_keyword() {
        let target = Query::new("--ＡＡＡ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Negative(Box::new(Condition::Keyword("-ＡＡＡ".into())))
        )
    }

    #[test]
    fn test_query_to_condition_two_keywords() {
        let target = Query::new("ＡＡＡ　ＢＢＢ".into());
        let actual = target.to_condition().unwrap();
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
    fn test_query_to_condition_two_exact_keywords() {
        let target = Query::new("\"ＡＡＡ　ＢＢＢ\"　\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.to_condition().unwrap();
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
    fn test_query_to_condition_two_negative_keywords() {
        let target = Query::new("-ＡＡＡ　-ＢＢＢ".into());
        let actual = target.to_condition().unwrap();
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
    fn test_query_to_condition_two_negative_exact_keywords() {
        let target = Query::new("-\"ＡＡＡ　ＢＢＢ\"　-\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.to_condition().unwrap();
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
    fn test_query_to_condition_two_keywords_with_or() {
        let target = Query::new("ＡＡＡ or　ＢＢＢ".into());
        let actual = target.to_condition().unwrap();
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
    fn test_query_to_condition_two_exact_keywords_with_or() {
        let target = Query::new("\"ＡＡＡ　ＢＢＢ\" or　\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.to_condition().unwrap();
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
    fn test_query_to_condition_two_negative_keywords_with_or() {
        let target = Query::new("-ＡＡＡ or　-ＢＢＢ".into());
        let actual = target.to_condition().unwrap();
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
    fn test_query_to_condition_two_negative_exact_keywords_with_or() {
        let target = Query::new("-\"ＡＡＡ　ＢＢＢ\" or　-\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.to_condition().unwrap();
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
    fn test_query_to_condition_two_keywords_with_and() {
        let target = Query::new("ＡＡＡ and　ＢＢＢ".into());
        let actual = target.to_condition().unwrap();
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
    fn test_query_to_condition_two_exact_keywords_with_and() {
        let target = Query::new("\"ＡＡＡ　ＢＢＢ\" and　\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.to_condition().unwrap();
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
    fn test_query_to_condition_two_negative_keywords_with_and() {
        let target = Query::new("-ＡＡＡ and　-ＢＢＢ".into());
        let actual = target.to_condition().unwrap();
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
    fn test_query_to_condition_two_negative_exact_keywords_with_and() {
        let target = Query::new("-\"ＡＡＡ　ＢＢＢ\" and　-\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.to_condition().unwrap();
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
    fn test_query_to_condition_multi_keywords_with_or_and() {
        let target = Query::new(
            "ＡＡＡ and　ＢＢＢ or ＣＣＣ ＤＤＤ and ＥＥＥ or ＦＦＦ or ＧＧＧ ＨＨＨ".into(),
        );
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            Condition::Operator(
                Operator::Or,
                vec![
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Keyword("ＡＡＡ".into()),
                            Condition::Keyword("ＢＢＢ".into())
                        ]
                    ),
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Keyword("ＣＣＣ".into()),
                            Condition::Keyword("ＤＤＤ".into()),
                            Condition::Keyword("ＥＥＥ".into())
                        ]
                    ),
                    Condition::Keyword("ＦＦＦ".into()),
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Keyword("ＧＧＧ".into()),
                            Condition::Keyword("ＨＨＨ".into())
                        ]
                    ),
                ]
            )
        )
    }

    #[test]
    fn test_query_to_condition_full_pattern() {
        let target = Query::new("　ＡＡＡ　　Ａｎｄ　-ＢＢＢ　ＡnＤ　ＣorＣ　　ｃｃｃ　Ｏr　　\"c1 and c2\"　　-\"c3 or c4\"　　ＤandＤ　anD　\"　ＥＥＥ　ＡNＤ　ＦＦＦ　\"　　ａnｄ　　-\"　ＧＧＧ　　oＲ　　ＨＨＨ　\"　　oＲ　　ＩＩＩ　and　".into());
        let actual = target.to_condition().unwrap();
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
                            Condition::Keyword("ｃｃｃ".into()),
                        ]
                    ),
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::ExactKeyword("c1 and c2".into()),
                            Condition::Negative(Box::new(Condition::ExactKeyword(
                                "c3 or c4".into()
                            ))),
                            Condition::Keyword("ＤandＤ".into()),
                            Condition::ExactKeyword(" ＥＥＥ ＡNＤ ＦＦＦ ".into()),
                            Condition::Negative(Box::new(Condition::ExactKeyword(
                                " ＧＧＧ  oＲ  ＨＨＨ ".into()
                            )))
                        ]
                    ),
                    Condition::Keyword("ＩＩＩ".into()),
                ]
            )
        )
    }
}
