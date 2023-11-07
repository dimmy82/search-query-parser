use crate::regex_approach::query::Query;
use crate::Condition::Not;
use crate::{Condition, ConditionOnTarget, Target};

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
                    // when only one child, remove self's operator layer
                    1 => conditions
                        .get(0)
                        .map(|condition| condition.clone())
                        .unwrap_or(Condition::None),
                    // when child is also a operator, and child's operator is equal to self's operator, remove child's operator layer
                    _ => Condition::Operator(
                        operator.clone(),
                        conditions
                            .into_iter()
                            .flat_map(|condition| match &condition {
                                Condition::Operator(inner_operator, inner_conditions) => {
                                    if &operator == inner_operator {
                                        inner_conditions.clone()
                                    } else {
                                        vec![condition]
                                    }
                                }
                                _ => vec![condition],
                            })
                            .collect(),
                    ),
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

    pub(crate) fn identify_target(self) -> ConditionOnTarget {
        match self {
            Condition::None => ConditionOnTarget::None,
            Condition::Keyword(keyword) => {
                let target_keyword = keyword
                    .split(&[':', '：'])
                    .filter(|it| it.is_empty() == false)
                    .collect::<Vec<&str>>();
                match target_keyword.len() {
                    2 => ConditionOnTarget::Keyword {
                        condition: target_keyword.get(1).unwrap().to_string(),
                        target: Some(parse_target(target_keyword.get(0).unwrap().to_string())),
                    },
                    _ => ConditionOnTarget::Keyword {
                        condition: keyword,
                        target: None,
                    },
                }
            }
            Condition::PhraseKeyword(phrase_keyword) => {
                todo!()
            }
            Not(condition) => {
                todo!()
            }
            Condition::Operator(operator, condition) => {
                todo!()
            }
        }
    }
}

fn parse_target(target_str: String) -> Target {
    let target_weight = target_str.split("^").collect::<Vec<&str>>();
    match target_weight.len() {
        2 => Target {
            name: target_weight.get(0).unwrap().to_string(),
            weight: target_weight
                .get(1)
                .unwrap()
                .to_string()
                .parse::<f32>()
                .map_or(None, |it| Some(it)),
        },
        _ => Target {
            name: target_str,
            weight: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod test_condition_simplify {
        use super::*;
        use crate::Operator;

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
        fn test_simplify_remove_same_operator_or_layer() {
            assert_eq!(
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("keyword1".into()),
                                Condition::Keyword("keyword2".into()),
                            ]
                        ),
                        Condition::PhraseKeyword("keyword3".into()),
                        Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::Keyword("keyword4".into()),
                                Condition::Keyword("keyword5".into()),
                            ]
                        ),
                        Condition::Keyword("keyword6".into()),
                        Condition::Not(Box::new(Condition::Keyword("keyword7".into())))
                    ]
                )
                .simplify(),
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("keyword1".into()),
                                Condition::Keyword("keyword2".into()),
                            ]
                        ),
                        Condition::PhraseKeyword("keyword3".into()),
                        Condition::Keyword("keyword4".into()),
                        Condition::Keyword("keyword5".into()),
                        Condition::Keyword("keyword6".into()),
                        Condition::Not(Box::new(Condition::Keyword("keyword7".into())))
                    ]
                ),
            )
        }

        #[test]
        fn test_simplify_remove_same_operator_and_layer() {
            assert_eq!(
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::Keyword("keyword1".into()),
                                Condition::Keyword("keyword2".into()),
                            ]
                        ),
                        Condition::PhraseKeyword("keyword3".into()),
                        Condition::Operator(
                            Operator::And,
                            vec![
                                Condition::Keyword("keyword4".into()),
                                Condition::Keyword("keyword5".into()),
                            ]
                        ),
                        Condition::Keyword("keyword6".into()),
                        Condition::Not(Box::new(Condition::Keyword("keyword7".into())))
                    ]
                )
                .simplify(),
                Condition::Operator(
                    Operator::And,
                    vec![
                        Condition::Operator(
                            Operator::Or,
                            vec![
                                Condition::Keyword("keyword1".into()),
                                Condition::Keyword("keyword2".into()),
                            ]
                        ),
                        Condition::PhraseKeyword("keyword3".into()),
                        Condition::Keyword("keyword4".into()),
                        Condition::Keyword("keyword5".into()),
                        Condition::Keyword("keyword6".into()),
                        Condition::Not(Box::new(Condition::Keyword("keyword7".into())))
                    ]
                ),
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

    mod test_identify_target {
        use crate::{Condition, ConditionOnTarget, Target};

        #[test]
        fn test_identify_target_on_none_condition() {
            assert_eq!(Condition::None.identify_target(), ConditionOnTarget::None)
        }

        #[test]
        fn test_no_identify_target_on_keyword_condition() {
            assert_eq!(
                Condition::Keyword("hoge".to_string()).identify_target(),
                ConditionOnTarget::Keyword {
                    condition: "hoge".to_string(),
                    target: None
                }
            )
        }

        #[test]
        fn test_identify_target_on_keyword_condition() {
            assert_eq!(
                Condition::Keyword("hoge:fuga".to_string()).identify_target(),
                ConditionOnTarget::Keyword {
                    condition: "fuga".to_string(),
                    target: Some(Target {
                        name: "hoge".to_string(),
                        weight: None
                    })
                }
            )
        }

        #[test]
        fn test_identify_target_on_keyword_condition_full_width() {
            assert_eq!(
                Condition::Keyword("hoge：fuga".to_string()).identify_target(),
                ConditionOnTarget::Keyword {
                    condition: "fuga".to_string(),
                    target: Some(Target {
                        name: "hoge".to_string(),
                        weight: None
                    })
                }
            )
        }

        #[test]
        fn test_identify_target_with_i32_weight_on_keyword_condition() {
            assert_eq!(
                Condition::Keyword("hoge^2:fuga".to_string()).identify_target(),
                ConditionOnTarget::Keyword {
                    condition: "fuga".to_string(),
                    target: Some(Target {
                        name: "hoge".to_string(),
                        weight: Some(2.0)
                    })
                }
            )
        }

        #[test]
        fn test_identify_target_with_f32_weight_on_keyword_condition() {
            assert_eq!(
                Condition::Keyword("hoge^0.2:fuga".to_string()).identify_target(),
                ConditionOnTarget::Keyword {
                    condition: "fuga".to_string(),
                    target: Some(Target {
                        name: "hoge".to_string(),
                        weight: Some(0.2)
                    })
                }
            )
        }

        #[test]
        fn test_identify_target_with_nan_weight_on_keyword_condition() {
            assert_eq!(
                Condition::Keyword("hoge^2a:fuga".to_string()).identify_target(),
                ConditionOnTarget::Keyword {
                    condition: "fuga".to_string(),
                    target: Some(Target {
                        name: "hoge".to_string(),
                        weight: None
                    })
                }
            )
        }

        #[test]
        fn test_identify_target_on_keyword_condition_invalid() {
            assert_eq!(
                Condition::Keyword("hoge:".to_string()).identify_target(),
                ConditionOnTarget::Keyword {
                    condition: "hoge:".to_string(),
                    target: None
                }
            );
            assert_eq!(
                Condition::Keyword(":hoge".to_string()).identify_target(),
                ConditionOnTarget::Keyword {
                    condition: ":hoge".to_string(),
                    target: None
                }
            );
            assert_eq!(
                Condition::Keyword("hoge:fuga:pico".to_string()).identify_target(),
                ConditionOnTarget::Keyword {
                    condition: "hoge:fuga:pico".to_string(),
                    target: None
                }
            )
        }
    }
}
