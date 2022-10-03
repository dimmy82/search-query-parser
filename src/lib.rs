mod bnf_approach;
mod regex_approach;

use crate::regex_approach::layered_query::LayeredQueries;
use crate::regex_approach::query::Query;
use eyre::Result;
use serde::Serialize;

pub fn parse_query_to_condition(query: &str) -> Result<Condition> {
    LayeredQueries::parse(Query::new(query.into()))?.to_condition()
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum Condition {
    None,
    Keyword(String),
    PhraseKeyword(String),
    Not(Box<Condition>),
    Operator(Operator, Vec<Condition>),
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum Operator {
    And,
    Or,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Condition, Operator};

    mod normal_query {
        use super::*;

        #[test]
        fn test_keywords_concat_with_spaces() {
            let actual = parse_query_to_condition("キーワード１ キーワード２").unwrap();
            assert_eq!(
                actual,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("キーワード１".into()),
                        Condition::Keyword("キーワード２".into())
                    ]
                )
            )
        }

        #[test]
        fn test_keywords_concat_with_and_or() {
            let actual =
                parse_query_to_condition("キーワード１ OR キーワード２ AND キーワード３").unwrap();
            assert_eq!(
                actual,
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("キーワード１".into()),
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("キーワード２".into()),
                                Condition::Keyword("キーワード３".into()),
                            ]
                        )
                    ]
                )
            )
        }

        #[test]
        fn test_brackets() {
            let actual =
                parse_query_to_condition("キーワード１ AND (キーワード２ OR キーワード３)")
                    .unwrap();
            assert_eq!(
                actual,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("キーワード１".into()),
                        Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::Keyword("キーワード２".into()),
                                Condition::Keyword("キーワード３".into()),
                            ]
                        )
                    ]
                )
            )
        }

        #[test]
        fn test_double_quote() {
            let actual = parse_query_to_condition(
                "\"キーワード１ AND (キーワード２ OR キーワード３)\" キーワード４",
            )
            .unwrap();
            assert_eq!(
                actual,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::PhraseKeyword(
                            "キーワード１ AND (キーワード２ OR キーワード３)".into()
                        ),
                        Condition::Keyword("キーワード４".into()),
                    ]
                )
            )
        }

        #[test]
        fn test_minus() {
            let actual = parse_query_to_condition(
                "-キーワード１ -\"キーワード２\" -(キーワード３ OR キーワード４)",
            )
            .unwrap();
            assert_eq!(
                actual,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Not(Box::new(Condition::Keyword("キーワード１".into()))),
                        Condition::Not(Box::new(Condition::PhraseKeyword("キーワード２".into()))),
                        Condition::Not(Box::new(Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::Keyword("キーワード３".into()),
                                Condition::Keyword("キーワード４".into())
                            ]
                        ))),
                    ]
                )
            )
        }

        #[test]
        fn test_full_pattern() {
            let actual = parse_query_to_condition(
                "(word１ and -word２) or ((\"phrase word １\" or -\"phrase word ２\") and -(\" a long phrase word \" or word３))",
            )
                .unwrap();
            assert_eq!(
                actual,
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
    }

    mod invalid_query {
        use super::*;

        #[test]
        fn test_empty_brackets() {
            let actual = parse_query_to_condition("A AND () AND B").unwrap();
            assert_eq!(
                actual,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("A".into()),
                        Condition::Keyword("B".into()),
                    ]
                )
            )
        }

        #[test]
        fn test_reverse_brackets() {
            let actual = parse_query_to_condition("A OR B) AND (C OR D").unwrap();
            assert_eq!(
                actual,
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("A".into()),
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("B".into()),
                                Condition::Keyword("C".into()),
                            ]
                        ),
                        Condition::Keyword("D".into()),
                    ]
                )
            )
        }

        #[test]
        fn test_missing_brackets() {
            let actual = parse_query_to_condition("(A OR B) AND (C").unwrap();
            assert_eq!(
                actual,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::Keyword("A".into()),
                                Condition::Keyword("B".into()),
                            ]
                        ),
                        Condition::Keyword("C".into()),
                    ]
                )
            )
        }

        #[test]
        fn test_invalid_nest_brackets() {
            let actual = parse_query_to_condition("(((A OR B))) AND C").unwrap();
            assert_eq!(
                actual,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::Keyword("A".into()),
                                Condition::Keyword("B".into()),
                            ]
                        ),
                        Condition::Keyword("C".into()),
                    ]
                )
            )
        }

        #[test]
        fn test_empty_phrase_keywords() {
            let actual = parse_query_to_condition("A AND \"\" AND B").unwrap();
            assert_eq!(
                actual,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("A".into()),
                        Condition::Keyword("B".into()),
                    ]
                )
            )
        }

        #[test]
        fn test_invalid_double_quote() {
            let actual = parse_query_to_condition("\"A\" OR \"B OR C").unwrap();
            assert_eq!(
                actual,
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::PhraseKeyword("A".into()),
                        Condition::Keyword("B".into()),
                        Condition::Keyword("C".into()),
                    ]
                )
            )
        }

        #[test]
        fn test_invalid_and_or() {
            let actual = parse_query_to_condition("A AND OR B").unwrap();
            assert_eq!(
                actual,
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("A".into()),
                        Condition::Keyword("B".into()),
                    ]
                )
            )
        }
    }

    mod effective_query {
        use super::*;

        #[test]
        fn test_unnecessary_nest_brackets() {
            let actual = parse_query_to_condition("(A OR (B OR C)) AND D").unwrap();
            assert_eq!(
                actual,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::Keyword("A".into()),
                                Condition::Keyword("B".into()),
                                Condition::Keyword("C".into()),
                            ]
                        ),
                        Condition::Keyword("D".into()),
                    ]
                )
            )
        }

        #[test]
        fn test_concat_brackets_without_space() {
            let actual = parse_query_to_condition("A(B OR C)D").unwrap();
            assert_eq!(
                actual,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("A".into()),
                        Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::Keyword("B".into()),
                                Condition::Keyword("C".into()),
                            ]
                        ),
                        Condition::Keyword("D".into()),
                    ]
                )
            )
        }

        #[test]
        fn test_concat_phrase_keywords_without_space() {
            let actual = parse_query_to_condition("A\"B\"C").unwrap();
            assert_eq!(
                actual,
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("A".into()),
                        Condition::PhraseKeyword("B".into()),
                        Condition::Keyword("C".into()),
                    ]
                )
            )
        }
    }
}
