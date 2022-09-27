use crate::condition::Condition::Not;
use crate::query::Query;
use serde::Serialize;

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

impl Condition {
    pub(crate) fn simplify(self) -> Self {
        match self {
            Not(condition) => match condition.simplify() {
                Condition::None => Condition::None,
                Not(condition) => condition.as_ref().clone(),
                condition => Not(Box::new(condition)),
            },
            Condition::Operator(operator, conditions) => {
                let conditions = conditions
                    .into_iter()
                    .filter_map(|condition| match condition.simplify() {
                        Condition::None => Option::None,
                        condition => Option::Some(condition),
                    })
                    .collect::<Vec<Condition>>();
                match conditions.len() {
                    0 => Condition::None,
                    1 => conditions
                        .get(0)
                        .map(|condition| condition.clone())
                        .unwrap_or(Condition::None),
                    _ => Condition::Operator(operator, conditions),
                }
            }
            Condition::PhraseKeyword(k) => {
                if Query::new(k.clone()).is_not_blank() {
                    Condition::PhraseKeyword(k)
                } else {
                    Condition::None
                }
            }
            Condition::Keyword(k) => {
                if Query::new(k.clone()).is_not_blank() {
                    Condition::Keyword(k)
                } else {
                    Condition::None
                }
            }
            _ => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod test_condition_simplify {
        use super::*;

        #[test]
        fn test_simplify_none() {
            assert_eq!(Condition::None.simplify(), Condition::None)
        }

        #[test]
        fn test_simplify_keyword() {
            assert_eq!(
                Condition::Keyword("keyword".into()).simplify(),
                Condition::Keyword("keyword".into())
            )
        }

        #[test]
        fn test_simplify_empty_keyword() {
            assert_eq!(Condition::Keyword("".into()).simplify(), Condition::None)
        }

        #[test]
        fn test_simplify_blank_keyword() {
            assert_eq!(
                Condition::Keyword(" 　　 ".into()).simplify(),
                Condition::None
            )
        }

        #[test]
        fn test_simplify_phrase_keyword() {
            assert_eq!(
                Condition::PhraseKeyword("phrase keyword".into()).simplify(),
                Condition::PhraseKeyword("phrase keyword".into())
            )
        }

        #[test]
        fn test_simplify_empty_phrase_keyword() {
            assert_eq!(
                Condition::PhraseKeyword("".into()).simplify(),
                Condition::None
            )
        }

        #[test]
        fn test_simplify_blank_phrase_keyword() {
            assert_eq!(
                Condition::PhraseKeyword(" 　　 ".into()).simplify(),
                Condition::None
            )
        }

        #[test]
        fn test_simplify_negative_none() {
            assert_eq!(
                Condition::Not(Box::new(Condition::None)).simplify(),
                Condition::None
            )
        }

        #[test]
        fn test_simplify_negative_keyword() {
            assert_eq!(
                Condition::Not(Box::new(Condition::Keyword("keyword".into()))).simplify(),
                Condition::Not(Box::new(Condition::Keyword("keyword".into())))
            )
        }

        #[test]
        fn test_simplify_negative_empty_keyword() {
            assert_eq!(
                Condition::Not(Box::new(Condition::Keyword("".into()))).simplify(),
                Condition::None
            )
        }

        #[test]
        fn test_simplify_negative_phrase_keyword() {
            assert_eq!(
                Condition::Not(Box::new(Condition::PhraseKeyword("phrase keyword".into())))
                    .simplify(),
                Condition::Not(Box::new(Condition::PhraseKeyword("phrase keyword".into())))
            )
        }

        #[test]
        fn test_simplify_negative_empty_phrase_keyword() {
            assert_eq!(
                Condition::Not(Box::new(Condition::PhraseKeyword("".into()))).simplify(),
                Condition::None
            )
        }

        #[test]
        fn test_simplify_negative_negative() {
            assert_eq!(
                Condition::Not(Box::new(Condition::Not(Box::new(Condition::Keyword(
                    "keyword".into()
                )))))
                .simplify(),
                Condition::Keyword("keyword".into())
            )
        }

        #[test]
        fn test_simplify_negative_negative_negative() {
            assert_eq!(
                Condition::Not(Box::new(Condition::Not(Box::new(Condition::Not(
                    Box::new(Condition::Keyword("keyword".into()))
                )))))
                .simplify(),
                Condition::Not(Box::new(Condition::Keyword("keyword".into())))
            )
        }

        #[test]
        fn test_simplify_negative_operator_and() {
            assert_eq!(
                Condition::Not(Box::new(Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::Not(Box::new(Condition::PhraseKeyword("phrase keyword".into())))
                    ]
                )))
                .simplify(),
                Condition::Not(Box::new(Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::Not(Box::new(Condition::PhraseKeyword("phrase keyword".into())))
                    ]
                )))
            )
        }

        #[test]
        fn test_simplify_negative_operator_or() {
            assert_eq!(
                Condition::Not(Box::new(Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::Not(Box::new(Condition::PhraseKeyword("phrase keyword".into())))
                    ]
                )))
                .simplify(),
                Condition::Not(Box::new(Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::Not(Box::new(Condition::PhraseKeyword("phrase keyword".into())))
                    ]
                )))
            )
        }

        #[test]
        fn test_simplify_operator_and_empty() {
            assert_eq!(
                Condition::Operator(Operator::And, vec![]).simplify(),
                Condition::None
            )
        }

        #[test]
        fn test_simplify_operator_and_with_only_none() {
            assert_eq!(
                Condition::Operator(Operator::And, vec![Condition::None]).simplify(),
                Condition::None
            )
        }

        #[test]
        fn test_simplify_operator_and_with_only_keyword() {
            assert_eq!(
                Condition::Operator(Operator::And, vec![Condition::Keyword("keyword".into())])
                    .simplify(),
                Condition::Keyword("keyword".into())
            )
        }

        #[test]
        fn test_simplify_operator_and_with_only_phrase_keyword() {
            assert_eq!(
                Condition::Operator(
                    Operator::And,
                    vec![Condition::PhraseKeyword("phrase keyword".into())]
                )
                .simplify(),
                Condition::PhraseKeyword("phrase keyword".into())
            )
        }

        #[test]
        fn test_simplify_operator_and_with_only_negative() {
            assert_eq!(
                Condition::Operator(
                    Operator::And,
                    vec![Condition::Not(Box::new(Condition::Keyword("not".into())))]
                )
                .simplify(),
                Condition::Not(Box::new(Condition::Keyword("not".into())))
            )
        }

        #[test]
        fn test_simplify_operator_and_with_only_operator() {
            assert_eq!(
                Condition::Operator(
                    Operator::And,
                    vec![Condition::Operator(
                        Operator::Or,
                        vec![
                            Condition::Keyword("keyword".into()),
                            Condition::PhraseKeyword("phrase keyword".into()),
                        ]
                    )]
                )
                .simplify(),
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::PhraseKeyword("phrase keyword".into()),
                    ]
                )
            )
        }

        #[test]
        fn test_simplify_operator_and_with_only_operator_recursively() {
            assert_eq!(
                Condition::Operator(
                    Operator::And,
                    vec![Condition::Operator(
                        Operator::Or,
                        vec![Condition::Keyword("keyword".into()),]
                    )]
                )
                .simplify(),
                Condition::Keyword("keyword".into())
            )
        }

        #[test]
        fn test_simplify_operator_and_with_all_conditions() {
            assert_eq!(
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::None,
                        Condition::PhraseKeyword("phrase keyword".into()),
                        Condition::None,
                        Condition::Not(Box::new(Condition::Keyword("not".into()))),
                        Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::Keyword("keyword".into()),
                                Condition::PhraseKeyword("phrase keyword".into()),
                            ]
                        )
                    ]
                )
                .simplify(),
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::PhraseKeyword("phrase keyword".into()),
                        Condition::Not(Box::new(Condition::Keyword("not".into()))),
                        Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::Keyword("keyword".into()),
                                Condition::PhraseKeyword("phrase keyword".into()),
                            ]
                        )
                    ]
                )
            )
        }

        #[test]
        fn test_simplify_operator_or_empty() {
            assert_eq!(
                Condition::Operator(Operator::Or, vec![]).simplify(),
                Condition::None
            )
        }

        #[test]
        fn test_simplify_operator_or_with_only_none() {
            assert_eq!(
                Condition::Operator(Operator::Or, vec![Condition::None]).simplify(),
                Condition::None
            )
        }

        #[test]
        fn test_simplify_operator_or_with_only_keyword() {
            assert_eq!(
                Condition::Operator(Operator::Or, vec![Condition::Keyword("keyword".into())])
                    .simplify(),
                Condition::Keyword("keyword".into())
            )
        }

        #[test]
        fn test_simplify_operator_or_with_only_phrase_keyword() {
            assert_eq!(
                Condition::Operator(
                    Operator::Or,
                    vec![Condition::PhraseKeyword("phrase keyword".into())]
                )
                .simplify(),
                Condition::PhraseKeyword("phrase keyword".into())
            )
        }

        #[test]
        fn test_simplify_operator_or_with_only_negative() {
            assert_eq!(
                Condition::Operator(
                    Operator::Or,
                    vec![Condition::Not(Box::new(Condition::Keyword("not".into())))]
                )
                .simplify(),
                Condition::Not(Box::new(Condition::Keyword("not".into())))
            )
        }

        #[test]
        fn test_simplify_operator_or_with_only_operator() {
            assert_eq!(
                Condition::Operator(
                    Operator::Or,
                    vec![Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Keyword("keyword".into()),
                            Condition::PhraseKeyword("phrase keyword".into()),
                        ]
                    )]
                )
                .simplify(),
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::PhraseKeyword("phrase keyword".into()),
                    ]
                )
            )
        }

        #[test]
        fn test_simplify_operator_or_with_only_operator_recursively() {
            assert_eq!(
                Condition::Operator(
                    Operator::Or,
                    vec![Condition::Operator(
                        Operator::And,
                        vec![Condition::Keyword("keyword".into()),]
                    )]
                )
                .simplify(),
                Condition::Keyword("keyword".into()),
            )
        }

        #[test]
        fn test_simplify_operator_or_with_all_conditions() {
            assert_eq!(
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::None,
                        Condition::PhraseKeyword("phrase keyword".into()),
                        Condition::None,
                        Condition::Not(Box::new(Condition::Keyword("not".into()))),
                        Condition::None,
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("keyword 1".into()),
                                Condition::PhraseKeyword("phrase keyword 1".into()),
                            ]
                        )
                    ]
                )
                .simplify(),
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::PhraseKeyword("phrase keyword".into()),
                        Condition::Not(Box::new(Condition::Keyword("not".into()))),
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("keyword 1".into()),
                                Condition::PhraseKeyword("phrase keyword 1".into()),
                            ]
                        )
                    ]
                )
            )
        }
    }
}
