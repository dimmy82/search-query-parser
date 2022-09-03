use crate::condition::Condition;
use crate::{filter_not_blank_query, match_to_number, Operator};
use eyre::Result;
use regex::{Captures, Regex};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct Query(String);

impl Query {
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn value(self) -> String {
        self.0
    }

    pub(crate) fn value_ref(&self) -> &str {
        self.0.as_str()
    }

    pub(crate) fn normalize(self) -> Self {
        Self(
            self.value()
                .replace("（", "(")
                .replace("）", ")")
                .replace("”", "\"")
                .replace("　", " "),
        )
    }

    pub(crate) fn is_not_blank(&self) -> bool {
        self.value_ref().replace(" ", "").is_empty() == false
    }

    pub(crate) fn to_condition(self) -> Result<(bool, Condition, bool)> {
        let query = self.normalize();
        let (mut query, negative_exact_keywords, exact_keywords) = query.extract_exact_keyword()?;

        query = Query::new(
            Regex::new(" +(?i)[A|Ａ](?i)[N|Ｎ](?i)[D|Ｄ] +")?
                .replace_all(query.value_ref(), |_: &Captures| String::from(" "))
                .to_string(),
        );

        let mut or_conditions = Vec::<Condition>::new();
        let (is_start_with_or, is_end_with_or) = match (
            Regex::new("^ *(?i)[O|Ｏ](?i)[R|Ｒ] *$")?.is_match(query.value_ref()),
            Regex::new("^ *(?i)[O|Ｏ](?i)[R|Ｒ] +")?.is_match(query.value_ref()),
            Regex::new(" +(?i)[O|Ｏ](?i)[R|Ｒ] *$")?.is_match(query.value_ref()),
        ) {
            (true, _, _) => (true, true),
            (false, is_start_with_or, is_end_with_or) => (is_start_with_or, is_end_with_or),
        };
        let or_queries = Regex::new(" +(?i)[O|Ｏ](?i)[R|Ｒ] +")?
            .split(query.value_ref())
            .into_iter()
            .collect::<Vec<&str>>();
        let and_regex = Regex::new(" +")?;
        or_queries.into_iter().for_each(|q| {
            let query = Query::new(q.into());
            if query.is_not_blank() {
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
        });

        return Ok((
            is_start_with_or,
            Condition::Operator(Operator::Or, or_conditions).simplify(),
            is_end_with_or,
        ));
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
                        match filter_not_blank_query(captures.get(1)) {
                            Some(q) => {
                                vec.push(q);
                                format!(" ({}:{}) ", prefix, vec.len())
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
                (Some(nek), _) => match_to_number(nek.get(1), |i| {
                    negative_exact_keywords.get(i - 1).map(|nek| {
                        Condition::Negative(Box::new(Condition::ExactKeyword(
                            nek.value_ref().to_string(),
                        )))
                    })
                }),
                (_, Some(ek)) => match_to_number(ek.get(1), |i| {
                    exact_keywords
                        .get(i - 1)
                        .map(|ek| Condition::ExactKeyword(ek.value_ref().to_string()))
                }),
                (None, None) => match (self.value_ref().len(), self.value_ref().starts_with("-")) {
                    (1, _) => Some(Condition::Keyword(self.value())),
                    (_, true) => Some(Condition::Negative(Box::new(Condition::Keyword(
                        self.value_ref()[1..self.value_ref().len()].into(),
                    )))),
                    _ => {
                        let operation_regexes = vec![
                            Regex::new("^(?i)[A|Ａ](?i)[N|Ｎ](?i)[D|Ｄ]$")?,
                            Regex::new("^(?i)[O|Ｏ](?i)[R|Ｒ]$")?,
                        ];
                        operation_regexes
                            .into_iter()
                            .find(|regex| regex.is_match(self.value_ref()))
                            .map(|_| None)
                            .unwrap_or(Some(Condition::Keyword(self.value())))
                    }
                },
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_replace_full_width_bracket_quotation_and_space() {
        let target =
            Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　（-（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　-”あああ　いいい”　ううう））　”　ＪＪＪ　”　-（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　".into());
        assert_eq!(
            target.normalize().value_ref(),
            " ＡＡＡ (\"１１１ ＣＣＣ\" (-( ＤＤＤ エエエ ) ＦＦＦ) ＧＧＧ (ＨＨＨ -\"あああ いいい\" ううう)) \" ＪＪＪ \" -(ＫＫＫ ( ) ＬＬＬ)  (ＭＭＭ) ２２２ "
        )
    }

    #[test]
    fn test_query_to_condition_only_space() {
        let target = Query::new("　   　".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(actual, (false, Condition::None, false))
    }

    #[test]
    fn test_query_to_condition_only_one_keyword() {
        let target = Query::new("ＡＡＡ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(actual, (false, Condition::Keyword("ＡＡＡ".into()), false))
    }

    #[test]
    fn test_query_to_condition_only_one_exact_keyword() {
        let target = Query::new("\"ＡＡＡ　ＢＢＢ\"".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::ExactKeyword("ＡＡＡ ＢＢＢ".into()),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_only_one_negative_keyword() {
        let target = Query::new("-ＡＡＡ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Negative(Box::new(Condition::Keyword("ＡＡＡ".into()))),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_only_one_negative_exact_keyword() {
        let target = Query::new("-\"ＡＡＡ　ＢＢＢ\"".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Negative(Box::new(Condition::ExactKeyword("ＡＡＡ ＢＢＢ".into()))),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_only_one_double_negative_keyword() {
        let target = Query::new("--ＡＡＡ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Negative(Box::new(Condition::Keyword("-ＡＡＡ".into()))),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_two_keywords() {
        let target = Query::new("ＡＡＡ　ＢＢＢ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("ＡＡＡ".into()),
                        Condition::Keyword("ＢＢＢ".into())
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_two_exact_keywords() {
        let target = Query::new("\"ＡＡＡ　ＢＢＢ\"　\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::ExactKeyword("ＡＡＡ ＢＢＢ".into()),
                        Condition::ExactKeyword("ＣＣＣ ＤＤＤ".into())
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_two_negative_keywords() {
        let target = Query::new("-ＡＡＡ　-ＢＢＢ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Negative(Box::new(Condition::Keyword("ＡＡＡ".into()))),
                        Condition::Negative(Box::new(Condition::Keyword("ＢＢＢ".into())))
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_two_negative_exact_keywords() {
        let target = Query::new("-\"ＡＡＡ　ＢＢＢ\"　-\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Negative(Box::new(Condition::ExactKeyword(
                            "ＡＡＡ ＢＢＢ".into()
                        ))),
                        Condition::Negative(Box::new(Condition::ExactKeyword(
                            "ＣＣＣ ＤＤＤ".into()
                        )))
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_multi_keywords() {
        let target = Query::new("ＡＡＡ　\"ＢＢＢ\"　-\"ＣＣＣ\"　-ＤＤＤ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("ＡＡＡ".into()),
                        Condition::ExactKeyword("ＢＢＢ".into()),
                        Condition::Negative(Box::new(Condition::ExactKeyword("ＣＣＣ".into()))),
                        Condition::Negative(Box::new(Condition::Keyword("ＤＤＤ".into())))
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_multi_keywords_without_space() {
        let target = Query::new("ＡＡＡ\"ＢＢＢ\"\"ｂｂｂ\"-\"ＣＣＣ\"-\"ｃｃｃ\"-ＤＤＤ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("ＡＡＡ".into()),
                        Condition::ExactKeyword("ＢＢＢ".into()),
                        Condition::ExactKeyword("ｂｂｂ".into()),
                        Condition::Negative(Box::new(Condition::ExactKeyword("ＣＣＣ".into()))),
                        Condition::Negative(Box::new(Condition::ExactKeyword("ｃｃｃ".into()))),
                        Condition::Negative(Box::new(Condition::Keyword("ＤＤＤ".into())))
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_two_keywords_with_or() {
        let target = Query::new("ＡＡＡ or　ＢＢＢ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("ＡＡＡ".into()),
                        Condition::Keyword("ＢＢＢ".into())
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_two_exact_keywords_with_or() {
        let target = Query::new("\"ＡＡＡ　ＢＢＢ\" or　\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::ExactKeyword("ＡＡＡ ＢＢＢ".into()),
                        Condition::ExactKeyword("ＣＣＣ ＤＤＤ".into())
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_two_negative_keywords_with_or() {
        let target = Query::new("-ＡＡＡ or　-ＢＢＢ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Negative(Box::new(Condition::Keyword("ＡＡＡ".into()))),
                        Condition::Negative(Box::new(Condition::Keyword("ＢＢＢ".into())))
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_two_negative_exact_keywords_with_or() {
        let target = Query::new("-\"ＡＡＡ　ＢＢＢ\" or　-\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Negative(Box::new(Condition::ExactKeyword(
                            "ＡＡＡ ＢＢＢ".into()
                        ))),
                        Condition::Negative(Box::new(Condition::ExactKeyword(
                            "ＣＣＣ ＤＤＤ".into()
                        )))
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_two_keywords_with_double_or() {
        let target = Query::new("ＡＡＡ　or　or　ＢＢＢ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("ＡＡＡ".into()),
                        Condition::Keyword("ＢＢＢ".into())
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_two_keywords_with_and() {
        let target = Query::new("ＡＡＡ and　ＢＢＢ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("ＡＡＡ".into()),
                        Condition::Keyword("ＢＢＢ".into())
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_two_exact_keywords_with_and() {
        let target = Query::new("\"ＡＡＡ　ＢＢＢ\" and　\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::ExactKeyword("ＡＡＡ ＢＢＢ".into()),
                        Condition::ExactKeyword("ＣＣＣ ＤＤＤ".into())
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_two_negative_keywords_with_and() {
        let target = Query::new("-ＡＡＡ and　-ＢＢＢ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Negative(Box::new(Condition::Keyword("ＡＡＡ".into()))),
                        Condition::Negative(Box::new(Condition::Keyword("ＢＢＢ".into())))
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_two_negative_exact_keywords_with_and() {
        let target = Query::new("-\"ＡＡＡ　ＢＢＢ\" and　-\"ＣＣＣ　ＤＤＤ\"".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Negative(Box::new(Condition::ExactKeyword(
                            "ＡＡＡ ＢＢＢ".into()
                        ))),
                        Condition::Negative(Box::new(Condition::ExactKeyword(
                            "ＣＣＣ ＤＤＤ".into()
                        )))
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_two_keywords_with_double_and() {
        let target = Query::new("ＡＡＡ　and　and　ＢＢＢ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("ＡＡＡ".into()),
                        Condition::Keyword("ＢＢＢ".into())
                    ]
                ),
                false
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
            (
                false,
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
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_two_keywords_with_double_and_or() {
        let target = Query::new("ＡＡＡ　and　or　and　or　ＢＢＢ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("ＡＡＡ".into()),
                        Condition::Keyword("ＢＢＢ".into())
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_and_or_in_exact_keyword() {
        let target = Query::new("ＡＡＡ　\"　and　ＢＢＢ　or　ＣＣＣ　and　\"　\"　or　ＤＤＤ　and　ＥＥＥ　or　\"　ＦＦＦ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("ＡＡＡ".into()),
                        Condition::ExactKeyword(" and ＢＢＢ or ＣＣＣ and ".into()),
                        Condition::ExactKeyword(" or ＤＤＤ and ＥＥＥ or ".into()),
                        Condition::Keyword("ＦＦＦ".into()),
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_full_pattern() {
        let target = Query::new("　ＡＡＡ　　Ａｎｄ　-ＢＢＢ　ＡnＤ　ＣorＣ　　ｃｃｃ　Ｏr　　\"c1 and c2\"　　-\"c3 or c4\"　　ＤandＤ　anD　\"　ＥＥＥ　ＡNＤ　ＦＦＦ　\"　　ａnｄ　　-\"　ＧＧＧ　　oＲ　　ＨＨＨ　\"　　oＲ　　ＩＩＩ　and　".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
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
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_start_end_with_and() {
        let target = Query::new("and ＡＡＡ　ＢＢＢ and".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("ＡＡＡ".into()),
                        Condition::Keyword("ＢＢＢ".into())
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_start_end_with_and_with_space() {
        let target = Query::new(" and ＡＡＡ　ＢＢＢ and ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                false,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("ＡＡＡ".into()),
                        Condition::Keyword("ＢＢＢ".into())
                    ]
                ),
                false
            )
        )
    }

    #[test]
    fn test_query_to_condition_start_end_with_or() {
        let target = Query::new("or ＡＡＡ　ＢＢＢ or".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                true,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("ＡＡＡ".into()),
                        Condition::Keyword("ＢＢＢ".into())
                    ]
                ),
                true
            )
        )
    }

    #[test]
    fn test_query_to_condition_start_end_with_or_with_space() {
        let target = Query::new(" or ＡＡＡ　ＢＢＢ or ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(
            actual,
            (
                true,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("ＡＡＡ".into()),
                        Condition::Keyword("ＢＢＢ".into())
                    ]
                ),
                true
            )
        )
    }

    #[test]
    fn test_query_to_condition_start_end_with_or_with_space_include_one_keyword() {
        let target = Query::new(" or ＡＡＡ or ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(actual, (true, Condition::Keyword("ＡＡＡ".into()), true))
    }

    #[test]
    fn test_query_to_condition_only_or() {
        let target = Query::new("or".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(actual, (true, Condition::None, true))
    }

    #[test]
    fn test_query_to_condition_only_or_with_space() {
        let target = Query::new(" or ".into());
        let actual = target.to_condition().unwrap();
        assert_eq!(actual, (true, Condition::None, true))
    }
}
