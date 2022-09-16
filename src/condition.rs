use crate::condition::Condition::Negative;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Condition {
    None,
    Keyword(String),
    ExactKeyword(String),
    Negative(Box<Condition>),
    Operator(Operator, Vec<Condition>),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Operator {
    And,
    Or,
}

impl Condition {
    pub(crate) fn simplify(self) -> Self {
        match self {
            Negative(condition) => match condition.simplify() {
                Condition::None => Condition::None,
                Negative(condition) => condition.as_ref().clone(),
                condition => Negative(Box::new(condition)),
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
        fn test_simplify_exact_keyword() {
            assert_eq!(
                Condition::ExactKeyword("exact keyword".into()).simplify(),
                Condition::ExactKeyword("exact keyword".into())
            )
        }

        #[test]
        fn test_simplify_negative_none() {
            assert_eq!(
                Condition::Negative(Box::new(Condition::None)).simplify(),
                Condition::None
            )
        }

        #[test]
        fn test_simplify_negative_keyword() {
            assert_eq!(
                Condition::Negative(Box::new(Condition::Keyword("keyword".into()))).simplify(),
                Condition::Negative(Box::new(Condition::Keyword("keyword".into())))
            )
        }

        #[test]
        fn test_simplify_negative_exact_keyword() {
            assert_eq!(
                Condition::Negative(Box::new(Condition::ExactKeyword("exact keyword".into())))
                    .simplify(),
                Condition::Negative(Box::new(Condition::ExactKeyword("exact keyword".into())))
            )
        }

        #[test]
        fn test_simplify_negative_negative() {
            assert_eq!(
                Condition::Negative(Box::new(Condition::Negative(Box::new(Condition::Keyword(
                    "keyword".into()
                )))))
                .simplify(),
                Condition::Keyword("keyword".into())
            )
        }

        #[test]
        fn test_simplify_negative_negative_negative() {
            assert_eq!(
                Condition::Negative(Box::new(Condition::Negative(Box::new(
                    Condition::Negative(Box::new(Condition::Keyword("keyword".into())))
                ))))
                .simplify(),
                Condition::Negative(Box::new(Condition::Keyword("keyword".into())))
            )
        }

        #[test]
        fn test_simplify_negative_operator_and() {
            assert_eq!(
                Condition::Negative(Box::new(Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::Negative(Box::new(Condition::ExactKeyword(
                            "exact keyword".into()
                        )))
                    ]
                )))
                .simplify(),
                Condition::Negative(Box::new(Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::Negative(Box::new(Condition::ExactKeyword(
                            "exact keyword".into()
                        )))
                    ]
                )))
            )
        }

        #[test]
        fn test_simplify_negative_operator_or() {
            assert_eq!(
                Condition::Negative(Box::new(Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::Negative(Box::new(Condition::ExactKeyword(
                            "exact keyword".into()
                        )))
                    ]
                )))
                .simplify(),
                Condition::Negative(Box::new(Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::Negative(Box::new(Condition::ExactKeyword(
                            "exact keyword".into()
                        )))
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
        fn test_simplify_operator_and_with_only_exact_keyword() {
            assert_eq!(
                Condition::Operator(
                    Operator::And,
                    vec![Condition::ExactKeyword("exact keyword".into())]
                )
                .simplify(),
                Condition::ExactKeyword("exact keyword".into())
            )
        }

        #[test]
        fn test_simplify_operator_and_with_only_negative() {
            assert_eq!(
                Condition::Operator(
                    Operator::And,
                    vec![Condition::Negative(Box::new(Condition::Keyword(
                        "negative".into()
                    )))]
                )
                .simplify(),
                Condition::Negative(Box::new(Condition::Keyword("negative".into())))
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
                            Condition::ExactKeyword("exact keyword".into()),
                        ]
                    )]
                )
                .simplify(),
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::ExactKeyword("exact keyword".into()),
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
                        Condition::ExactKeyword("exact keyword".into()),
                        Condition::None,
                        Condition::Negative(Box::new(Condition::Keyword("negative".into()))),
                        Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::Keyword("keyword".into()),
                                Condition::ExactKeyword("exact keyword".into()),
                            ]
                        )
                    ]
                )
                .simplify(),
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::ExactKeyword("exact keyword".into()),
                        Condition::Negative(Box::new(Condition::Keyword("negative".into()))),
                        Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::Keyword("keyword".into()),
                                Condition::ExactKeyword("exact keyword".into()),
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
        fn test_simplify_operator_or_with_only_exact_keyword() {
            assert_eq!(
                Condition::Operator(
                    Operator::Or,
                    vec![Condition::ExactKeyword("exact keyword".into())]
                )
                .simplify(),
                Condition::ExactKeyword("exact keyword".into())
            )
        }

        #[test]
        fn test_simplify_operator_or_with_only_negative() {
            assert_eq!(
                Condition::Operator(
                    Operator::Or,
                    vec![Condition::Negative(Box::new(Condition::Keyword(
                        "negative".into()
                    )))]
                )
                .simplify(),
                Condition::Negative(Box::new(Condition::Keyword("negative".into())))
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
                            Condition::ExactKeyword("exact keyword".into()),
                        ]
                    )]
                )
                .simplify(),
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::ExactKeyword("exact keyword".into()),
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
                        Condition::ExactKeyword("exact keyword".into()),
                        Condition::None,
                        Condition::Negative(Box::new(Condition::Keyword("negative".into()))),
                        Condition::None,
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("keyword 1".into()),
                                Condition::ExactKeyword("exact keyword 1".into()),
                            ]
                        )
                    ]
                )
                .simplify(),
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("keyword".into()),
                        Condition::ExactKeyword("exact keyword".into()),
                        Condition::Negative(Box::new(Condition::Keyword("negative".into()))),
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("keyword 1".into()),
                                Condition::ExactKeyword("exact keyword 1".into()),
                            ]
                        )
                    ]
                )
            )
        }
    }
}
