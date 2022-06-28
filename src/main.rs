use eyre::Result;
use regex::{Captures, Match, Regex};

fn main() {
    println!("Hello, world!");
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum Layer {
    Query(Query),
    Bracket(Vec<Layer>),
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum Condition {
    Keyword(String),
    ExactKeyword(String),
    Not(Box<Condition>),
    And(Vec<Condition>),
    Or(Vec<Condition>),
}

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

    pub fn value(&self) -> &str {
        self.0.as_str()
    }

    fn is_not_blank(&self) -> bool {
        self.0.replace(" ", "").is_empty() == false
    }

    fn layered_by_bracket(self) -> Result<Vec<Layer>> {
        fn pick_layer_by_bracket(
            query: String, bracket_queries: &mut Vec<Query>,
        ) -> Result<String> {
            let regex_bracket = Regex::new(r"\(([^\(\)]*)\)")?;
            let innermost_bracket_removed_query = regex_bracket
                .replace_all(query.as_str(), |captures: &Captures| {
                    match captures.get(1) {
                        Some(m) => {
                            let q = Query::new(m.as_str().into());
                            match q.is_not_blank() {
                                true => {
                                    bracket_queries.push(q);
                                    format!("（{}）", bracket_queries.len())
                                }
                                false => String::from(""),
                            }
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
        let all_brackets_removed_query = pick_layer_by_bracket(self.0, &mut bracket_queries)?;

        fn filter_not_blank_query(regex_match: Option<Match>) -> Option<Query> {
            regex_match
                .map(|m| Query::new(m.as_str().into()))
                .filter(|q| q.is_not_blank())
        }

        fn combine_layered_query(query: Query, bracket_queries: &Vec<Query>) -> Result<Vec<Layer>> {
            let regex_layered_by_bracket = Regex::new(r"([^\(\)]*)\((\d)\)([^\(\)]*)")?;
            let mut layered_queries = Vec::<Layer>::new();
            regex_layered_by_bracket
                .captures_iter(query.value())
                .for_each(|captures| {
                    filter_not_blank_query(captures.get(1))
                        .map(|q| layered_queries.push(Layer::Query(q)));
                    captures
                        .get(2)
                        .map(|m| m.as_str().parse::<usize>())
                        .map(|index| {
                            index.map(|i| {
                                bracket_queries.get(i - 1).map(|q: &Query| {
                                    combine_layered_query(q.clone(), bracket_queries)
                                        .map(|v| layered_queries.push(Layer::Bracket(v)))
                                })
                            })
                        });
                    filter_not_blank_query(captures.get(3))
                        .map(|q| layered_queries.push(Layer::Query(q)));
                });
            if layered_queries.is_empty() {
                layered_queries.push(Layer::Query(query))
            }
            Ok(layered_queries)
        }

        Ok(combine_layered_query(
            Query::new(all_brackets_removed_query),
            &bracket_queries,
        )?)
    }

    // k1 or (k2 and (-k3 or -k4))
    fn parse(self) -> Result<Condition> {
        let layers = self.layered_by_bracket()?;
        layers.into_iter().for_each(|layer| {});
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_full_width_bracket_quotation_and_space_when_new() {
        let target =
            Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　（（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　”あああ　いいい”　ううう））　”　ＪＪＪ　”　（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　".into());
        assert_eq!(
            target.value(),
            " ＡＡＡ (\"１１１ ＣＣＣ\" (( ＤＤＤ エエエ ) ＦＦＦ) ＧＧＧ (ＨＨＨ \"あああ いいい\" ううう)) \" ＪＪＪ \" (ＫＫＫ ( ) ＬＬＬ)  (ＭＭＭ) ２２２ "
        )
    }

    #[test]
    fn test_layered_by_bracket() {
        let target =
            Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　（（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　”あああ　いいい”　ううう））　”　ＪＪＪ　”　（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　".into());
        assert_eq!(
            target.layered_by_bracket().unwrap(),
            vec![
                Layer::Query(Query::new(" ＡＡＡ ".into())),
                Layer::Bracket(vec![
                    Layer::Query(Query::new("\"１１１ ＣＣＣ\" ".into())),
                    Layer::Bracket(vec![
                        Layer::Bracket(vec![Layer::Query(Query::new(" ＤＤＤ エエエ ".into())),]),
                        Layer::Query(Query::new(" ＦＦＦ".into())),
                    ]),
                    Layer::Query(Query::new(" ＧＧＧ ".into())),
                    Layer::Bracket(vec![Layer::Query(Query::new(
                        "ＨＨＨ \"あああ いいい\" ううう".into()
                    )),])
                ]),
                Layer::Query(Query::new(" \" ＪＪＪ \" ".into())),
                Layer::Bracket(vec![Layer::Query(Query::new("ＫＫＫ  ＬＬＬ".into())),]),
                Layer::Bracket(vec![Layer::Query(Query::new("ＭＭＭ".into())),]),
                Layer::Query(Query::new(" ２２２ ".into())),
            ]
        )
    }

    #[test]
    fn test_parse() {
        let target =
            Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　（（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　”あああ　いいい”　ううう））　”　ＪＪＪ　”　（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　".into());
        assert_eq!(
            target.parse().unwrap(),
            Condition::Or(vec![
                Condition::Keyword("ＡＡＡ".into()),
                Condition::Or(vec![
                    Condition::ExactKeyword("１１１ ＣＣＣ".into()),
                    Condition::Or(vec![
                        Condition::Or(vec![
                            Condition::Keyword("ＤＤＤ".into()),
                            Condition::Keyword("エエエ".into()),
                        ]),
                        Condition::Keyword("ＦＦＦ".into()),
                    ]),
                    Condition::Keyword("ＧＧＧ".into()),
                    Condition::Or(vec![
                        Condition::Keyword("ＨＨＨ".into()),
                        Condition::ExactKeyword("あああ いいい".into()),
                        Condition::Keyword("ううう".into()),
                    ]),
                ]),
                Condition::ExactKeyword(" ＪＪＪ ".into()),
                Condition::Or(vec![
                    Condition::Keyword("ＫＫＫ".into()),
                    Condition::Keyword("ＬＬＬ".into()),
                ]),
                Condition::Or(vec![Condition::Keyword("ＭＭＭ".into()),]),
                Condition::Keyword("２２２".into()),
            ])
        )
    }
}
