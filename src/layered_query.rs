use crate::condition::Condition;
use crate::query::Query;
use eyre::Result;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LayeredQueries(Vec<LayeredQuery>);

impl LayeredQueries {
    pub(crate) fn new(layered_queries: Vec<LayeredQuery>) -> Self {
        Self(layered_queries)
    }

    // k1 or (k2 and (-k3 or -k4))
    fn parse_to_condition(self) -> Result<Condition> {
        let layered_query_count = self.0.iter().count();
        self.0
            .into_iter()
            .enumerate()
            .for_each(|(index, layered_query)| match index {
                i if i < layered_query_count - 1 => {}
                _ => {}
            });
        unimplemented!()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LayeredQuery {
    Query(Query),
    Bracket(LayeredQueries),
    NegativeBracket(LayeredQueries),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::Query;
    use crate::Operator;

    #[test]
    fn test_layered_queries_parse_to_condition() {
        let target =
            Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　（-（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　-”あああ　いいい”　ううう））　”　ＪＪＪ　”　-（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　".into());
        assert_eq!(
            target
                .layered_by_bracket()
                .unwrap()
                .parse_to_condition()
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
