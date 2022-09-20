use crate::condition::{Condition, Operator};
use crate::{regex_match_not_blank_query, regex_match_number};
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

    pub(crate) fn normalize_double_quotation(self) -> Self {
        Self(self.value().replace("”", "\""))
    }

    pub(crate) fn normalize_symbols_except_double_quotation(self) -> Self {
        Self(
            self.value()
                .replace("（", "(")
                .replace("）", ")")
                .replace("　", " "),
        )
    }

    fn remove_double_quotation(self) -> Self {
        Self(self.value().replace("\"", ""))
    }

    pub(crate) fn is_not_blank(&self) -> bool {
        self.value_ref().replace(" ", "").is_empty() == false
    }

    pub(crate) fn extract_phrase_keywords(self) -> Result<(Self, Vec<Query>, Vec<Query>)> {
        let mut query = self;
        let mut negative_phrase_keywords = Vec::<Query>::new();
        let mut phrase_keywords = Vec::<Query>::new();
        vec![
            (
                Regex::new("-\"([^\"]*)\"")?,
                &mut negative_phrase_keywords,
                "NPK",
            ),
            (Regex::new("\"([^\"]*)\"")?, &mut phrase_keywords, "PK"),
        ]
        .iter_mut()
        .for_each(|(regex, vec, prefix)| {
            query = Query::new(
                regex
                    .replace_all(query.value_ref(), |captures: &Captures| {
                        match regex_match_not_blank_query(captures.get(1)) {
                            Some(q) => {
                                vec.push(q);
                                format!(" ”{}:{}” ", prefix, vec.len())
                            }
                            None => String::from(""),
                        }
                    })
                    .to_string(),
            )
        });
        query = query.remove_double_quotation();
        Ok((query, negative_phrase_keywords, phrase_keywords))
    }

    pub(crate) fn combine_phrase_keywords(
        self, negative_phrase_keywords: &Vec<Query>, phrase_keywords: &Vec<Query>,
    ) -> Result<Self> {
        let mut query = self;
        vec![
            (Regex::new(r"”NPK:(\d+)”")?, negative_phrase_keywords, "-"),
            (Regex::new(r"”PK:(\d+)”")?, phrase_keywords, ""),
        ]
        .into_iter()
        .for_each(|(regex, vec, prefix)| {
            query = Query::new(
                regex
                    .replace_all(query.value_ref(), |captures: &Captures| {
                        regex_match_number(captures.get(1), |i| {
                            vec.get(i - 1)
                                .map(|q| format!("{}\"{}\"", prefix, q.value_ref()))
                        })
                        .unwrap_or("".into())
                    })
                    .into(),
            );
        });
        Ok(query)
    }

    pub(crate) fn to_condition(self) -> Result<(bool, Condition, bool)> {
        let (mut query, negative_phrase_keywords, phrase_keywords) =
            self.extract_phrase_keywords()?;

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
                            .keyword_condition(&negative_phrase_keywords, &phrase_keywords)
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

    fn keyword_condition(
        self, negative_phrase_keywords: &Vec<Query>, phrase_keywords: &Vec<Query>,
    ) -> Result<Option<Condition>> {
        Ok(
            match (
                Regex::new(r"^”NPK:(\d+)”$")?.captures(self.value_ref()),
                Regex::new(r"^”PK:(\d+)”$")?.captures(self.value_ref()),
            ) {
                (Some(npk), _) => regex_match_number(npk.get(1), |i| {
                    negative_phrase_keywords.get(i - 1).map(|npk| {
                        Condition::Not(Box::new(Condition::PhraseKeyword(npk.value_ref().into())))
                    })
                }),
                (_, Some(pk)) => regex_match_number(pk.get(1), |i| {
                    phrase_keywords
                        .get(i - 1)
                        .map(|pk| Condition::PhraseKeyword(pk.value_ref().into()))
                }),
                (None, None) => match (self.value_ref().len(), self.value_ref().starts_with("-")) {
                    (1, _) => Some(Condition::Keyword(self.value())),
                    (_, true) => Some(Condition::Not(Box::new(Condition::Keyword(
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

    mod test_query_normalize {
        use super::*;

        #[test]
        fn test_normalize_replace_full_width_double_quotation() {
            let target =
                Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　（-（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　-”あああ　いいい”　ううう））　”　ＪＪＪ　”　-（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　".into());
            assert_eq!(
                target.normalize_double_quotation().value_ref(),
                "　ＡＡＡ　（\"１１１　ＣＣＣ\"　（-（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　-\"あああ　いいい\"　ううう））　\"　ＪＪＪ　\"　-（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　"
            )
        }

        #[test]
        fn test_normalize_replace_full_width_bracket_and_space() {
            let target =
                Query::new("　ＡＡＡ　（”１１１　ＣＣＣ”　（-（　ＤＤＤ　エエエ　）　ＦＦＦ）　ＧＧＧ　（ＨＨＨ　-”あああ　いいい”　ううう））　”　ＪＪＪ　”　-（ＫＫＫ　（　）　ＬＬＬ）　　（ＭＭＭ）　２２２　".into());
            assert_eq!(
                target.normalize_symbols_except_double_quotation().value_ref(),
                " ＡＡＡ (”１１１ ＣＣＣ” (-( ＤＤＤ エエエ ) ＦＦＦ) ＧＧＧ (ＨＨＨ -”あああ いいい” ううう)) ” ＪＪＪ ” -(ＫＫＫ ( ) ＬＬＬ)  (ＭＭＭ) ２２２ "
            )
        }
    }

    mod test_extract_phrase_keywords {
        use super::*;

        #[test]
        fn test_extract_phrase_keywords_empty() {
            let target = Query::new("Ａ１ Ａ２".into());
            let (query, negative_phrase_keywords, phrase_keywords) =
                target.extract_phrase_keywords().unwrap();
            assert_eq!(query, Query::new("Ａ１ Ａ２".into()));
            assert_eq!(negative_phrase_keywords, vec![]);
            assert_eq!(phrase_keywords, vec![])
        }

        #[test]
        fn test_extract_phrase_keywords_one_phrase_keyword() {
            let target = Query::new("Ａ１ \"Ｐ１\" Ａ２".into());
            let (query, negative_phrase_keywords, phrase_keywords) =
                target.extract_phrase_keywords().unwrap();
            assert_eq!(query, Query::new("Ａ１  ”PK:1”  Ａ２".into()));
            assert_eq!(negative_phrase_keywords, vec![]);
            assert_eq!(phrase_keywords, vec![Query::new("Ｐ１".into())])
        }

        #[test]
        fn test_extract_phrase_keywords_one_negative_phrase_keyword() {
            let target = Query::new("Ａ１ -\"ＮＰ１\" Ａ２".into());
            let (query, negative_phrase_keywords, phrase_keywords) =
                target.extract_phrase_keywords().unwrap();
            assert_eq!(query, Query::new("Ａ１  ”NPK:1”  Ａ２".into()));
            assert_eq!(negative_phrase_keywords, vec![Query::new("ＮＰ１".into())]);
            assert_eq!(phrase_keywords, vec![])
        }

        #[test]
        fn test_extract_phrase_keywords_multi_phrase_keywords_and_negative_phrase_keywords() {
            let target = Query::new("-\"ＮＰ１\" Ａ１ or \"Ｐ３\" and -\"ＮＰ３\" -\"ＮＰ２\" \"Ｐ２\" or Ａ２ and \"Ｐ１\"".into());
            let (query, negative_phrase_keywords, phrase_keywords) =
                target.extract_phrase_keywords().unwrap();
            assert_eq!(query, Query::new(" ”NPK:1”  Ａ１ or  ”PK:1”  and  ”NPK:2”   ”NPK:3”   ”PK:2”  or Ａ２ and  ”PK:3” ".into()));
            assert_eq!(
                negative_phrase_keywords,
                vec![
                    Query::new("ＮＰ１".into()),
                    Query::new("ＮＰ３".into()),
                    Query::new("ＮＰ２".into())
                ]
            );
            assert_eq!(
                phrase_keywords,
                vec![
                    Query::new("Ｐ３".into()),
                    Query::new("Ｐ２".into()),
                    Query::new("Ｐ１".into())
                ]
            )
        }

        #[test]
        fn test_extract_phrase_keywords_special_symbol_in_phrase_keyword() {
            let target = Query::new("Ａ１ \" Ｐ１ and Ｐ２ -(Ｐ３ or Ｐ４) \" Ａ２".into());
            let (query, negative_phrase_keywords, phrase_keywords) =
                target.extract_phrase_keywords().unwrap();
            assert_eq!(query, Query::new("Ａ１  ”PK:1”  Ａ２".into()));
            assert_eq!(negative_phrase_keywords, vec![]);
            assert_eq!(
                phrase_keywords,
                vec![Query::new(" Ｐ１ and Ｐ２ -(Ｐ３ or Ｐ４) ".into())]
            )
        }

        #[test]
        fn test_extract_phrase_keywords_full_width_special_symbol_in_phrase_keyword() {
            let target =
                Query::new("Ａ１ \"　Ｐ１　ａｎｄ　Ｐ２　−（Ｐ３　ｏｒ　Ｐ４）　\" Ａ２".into());
            let (query, negative_phrase_keywords, phrase_keywords) =
                target.extract_phrase_keywords().unwrap();
            assert_eq!(query, Query::new("Ａ１  ”PK:1”  Ａ２".into()));
            assert_eq!(negative_phrase_keywords, vec![]);
            assert_eq!(
                phrase_keywords,
                vec![Query::new(
                    "　Ｐ１　ａｎｄ　Ｐ２　−（Ｐ３　ｏｒ　Ｐ４）　".into()
                )]
            )
        }

        #[test]
        fn test_extract_phrase_keywords_special_symbol_in_negative_phrase_keyword() {
            let target = Query::new("Ａ１ -\" Ｐ１ and Ｐ２ -(Ｐ３ or Ｐ４) \" Ａ２".into());
            let (query, negative_phrase_keywords, phrase_keywords) =
                target.extract_phrase_keywords().unwrap();
            assert_eq!(query, Query::new("Ａ１  ”NPK:1”  Ａ２".into()));
            assert_eq!(
                negative_phrase_keywords,
                vec![Query::new(" Ｐ１ and Ｐ２ -(Ｐ３ or Ｐ４) ".into())]
            );
            assert_eq!(phrase_keywords, vec![])
        }

        #[test]
        fn test_extract_phrase_keywords_full_width_special_symbol_in_negative_phrase_keyword() {
            let target =
                Query::new("Ａ１ -\"　Ｐ１　ａｎｄ　Ｐ２　−（Ｐ３　ｏｒ　Ｐ４）　\" Ａ２".into());
            let (query, negative_phrase_keywords, phrase_keywords) =
                target.extract_phrase_keywords().unwrap();
            assert_eq!(query, Query::new("Ａ１  ”NPK:1”  Ａ２".into()));
            assert_eq!(
                negative_phrase_keywords,
                vec![Query::new(
                    "　Ｐ１　ａｎｄ　Ｐ２　−（Ｐ３　ｏｒ　Ｐ４）　".into()
                )]
            );
            assert_eq!(phrase_keywords, vec![])
        }

        #[test]
        fn test_extract_phrase_keywords_excess_double_quotation_1() {
            let target = Query::new("Ａ１ \"Ｐ１\" -\"ＮＰ１\" \" Ａ２".into());
            let (query, negative_phrase_keywords, phrase_keywords) =
                target.extract_phrase_keywords().unwrap();
            assert_eq!(query, Query::new("Ａ１  ”PK:1”   ”NPK:1”   Ａ２".into()));
            assert_eq!(negative_phrase_keywords, vec![Query::new("ＮＰ１".into())]);
            assert_eq!(phrase_keywords, vec![Query::new("Ｐ１".into())])
        }

        #[test]
        fn test_extract_phrase_keywords_excess_double_quotation_2() {
            let target = Query::new("Ａ１ \"Ｐ１\" -\"ＮＰ１\" -\"Ａ２".into());
            let (query, negative_phrase_keywords, phrase_keywords) =
                target.extract_phrase_keywords().unwrap();
            assert_eq!(query, Query::new("Ａ１  ”PK:1”   ”NPK:1”  -Ａ２".into()));
            assert_eq!(negative_phrase_keywords, vec![Query::new("ＮＰ１".into())]);
            assert_eq!(phrase_keywords, vec![Query::new("Ｐ１".into())])
        }
    }

    mod test_combine_phrase_keywords {
        use super::*;

        #[test]
        fn test_combine_phrase_keywords_none() {
            let target = Query::new("Ａ１ and Ａ２ or Ａ３".into());
            let query = target
                .combine_phrase_keywords(
                    &vec![
                        Query::new("否定的な連続キーワード１".into()),
                        Query::new("否定的な連続キーワード２".into()),
                    ],
                    &vec![
                        Query::new("連続キーワード１".into()),
                        Query::new("連続キーワード２".into()),
                    ],
                )
                .unwrap();
            assert_eq!(query, Query::new("Ａ１ and Ａ２ or Ａ３".into()))
        }

        #[test]
        fn test_combine_phrase_keywords_multi_phrase_keywords_and_negative_phrase_keywords() {
            let target = Query::new("Ａ１ ”PK:1” and ”NPK:1” Ａ２ ”PK:2” or ”NPK:2” Ａ３".into());
            let query = target
                .combine_phrase_keywords(
                    &vec![
                        Query::new("否定的な連続キーワード１".into()),
                        Query::new("否定的な連続キーワード２".into()),
                    ],
                    &vec![
                        Query::new("連続キーワード１".into()),
                        Query::new("連続キーワード２".into()),
                    ],
                )
                .unwrap();
            assert_eq!(query,Query::new("Ａ１ \"連続キーワード１\" and -\"否定的な連続キーワード１\" Ａ２ \"連続キーワード２\" or -\"否定的な連続キーワード２\" Ａ３".into()))
        }

        #[test]
        fn test_combine_phrase_keywords_not_exists() {
            let target = Query::new("Ａ１ ”PK:1” and ”NPK:1” Ａ２ ”PK:2” or ”NPK:2” Ａ３".into());
            let query = target
                .combine_phrase_keywords(
                    &vec![Query::new("否定的な連続キーワード１".into())],
                    &vec![Query::new("連続キーワード１".into())],
                )
                .unwrap();
            assert_eq!(
                query,
                Query::new(
                    "Ａ１ \"連続キーワード１\" and -\"否定的な連続キーワード１\" Ａ２  or  Ａ３"
                        .into()
                )
            )
        }
    }

    mod test_query_to_condition {
        use super::*;

        #[test]
        fn test_query_to_condition_only_space() {
            let target = Query::new(" ".into());
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
        fn test_query_to_condition_only_one_phrase_keyword() {
            let target = Query::new("\"ＡＡＡ ＢＢＢ\"".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::PhraseKeyword("ＡＡＡ ＢＢＢ".into()),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_only_one_phrase_keyword_include_special_word() {
            let target = Query::new("\" Ｐ１ and Ｐ２ -(Ｐ３ or Ｐ４) \"".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::PhraseKeyword(" Ｐ１ and Ｐ２ -(Ｐ３ or Ｐ４) ".into()),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_only_one_phrase_keyword_include_full_width_special_word() {
            let target = Query::new("\"　Ｐ１　ａｎｄ　Ｐ２　−（Ｐ３　ｏｒ　Ｐ４）　\"".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::PhraseKeyword(
                        "　Ｐ１　ａｎｄ　Ｐ２　−（Ｐ３　ｏｒ　Ｐ４）　".into()
                    ),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_ten_phrase_keywords() {
            let target = Query::new("\"ＡＡＡ１\" \"ＡＡＡ２\" \"ＡＡＡ３\" \"ＡＡＡ４\" \"ＡＡＡ５\" \"ＡＡＡ６\" \"ＡＡＡ７\" \"ＡＡＡ８\" \"ＡＡＡ９\" \"ＡＡＡ１０\"".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::PhraseKeyword("ＡＡＡ１".into()),
                            Condition::PhraseKeyword("ＡＡＡ２".into()),
                            Condition::PhraseKeyword("ＡＡＡ３".into()),
                            Condition::PhraseKeyword("ＡＡＡ４".into()),
                            Condition::PhraseKeyword("ＡＡＡ５".into()),
                            Condition::PhraseKeyword("ＡＡＡ６".into()),
                            Condition::PhraseKeyword("ＡＡＡ７".into()),
                            Condition::PhraseKeyword("ＡＡＡ８".into()),
                            Condition::PhraseKeyword("ＡＡＡ９".into()),
                            Condition::PhraseKeyword("ＡＡＡ１０".into()),
                        ]
                    ),
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
                    Condition::Not(Box::new(Condition::Keyword("ＡＡＡ".into()))),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_only_one_negative_phrase_keyword() {
            let target = Query::new("-\"ＡＡＡ ＢＢＢ\"".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Not(Box::new(Condition::PhraseKeyword("ＡＡＡ ＢＢＢ".into()))),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_only_one_negative_phrase_keyword_include_special_word() {
            let target = Query::new("-\" ＮＰ１ and ＮＰ２ -(ＮＰ３ or ＮＰ４) \"".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Not(Box::new(Condition::PhraseKeyword(
                        " ＮＰ１ and ＮＰ２ -(ＮＰ３ or ＮＰ４) ".into()
                    ))),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_only_one_negative_phrase_keyword_include_full_width_special_word(
        ) {
            let target =
                Query::new("-\"　ＮＰ１　ａｎｄ　ＮＰ２　−（ＮＰ３　ｏｒ　ＮＰ４）　\"".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Not(Box::new(Condition::PhraseKeyword(
                        "　ＮＰ１　ａｎｄ　ＮＰ２　−（ＮＰ３　ｏｒ　ＮＰ４）　".into()
                    ))),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_ten_negative_phrase_keywords() {
            let target = Query::new("-\"ＡＡＡ１\" -\"ＡＡＡ２\" -\"ＡＡＡ３\" -\"ＡＡＡ４\" -\"ＡＡＡ５\" -\"ＡＡＡ６\" -\"ＡＡＡ７\" -\"ＡＡＡ８\" -\"ＡＡＡ９\" -\"ＡＡＡ１０\"".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Not(Box::new(Condition::PhraseKeyword("ＡＡＡ１".into()))),
                            Condition::Not(Box::new(Condition::PhraseKeyword("ＡＡＡ２".into()))),
                            Condition::Not(Box::new(Condition::PhraseKeyword("ＡＡＡ３".into()))),
                            Condition::Not(Box::new(Condition::PhraseKeyword("ＡＡＡ４".into()))),
                            Condition::Not(Box::new(Condition::PhraseKeyword("ＡＡＡ５".into()))),
                            Condition::Not(Box::new(Condition::PhraseKeyword("ＡＡＡ６".into()))),
                            Condition::Not(Box::new(Condition::PhraseKeyword("ＡＡＡ７".into()))),
                            Condition::Not(Box::new(Condition::PhraseKeyword("ＡＡＡ８".into()))),
                            Condition::Not(Box::new(Condition::PhraseKeyword("ＡＡＡ９".into()))),
                            Condition::Not(Box::new(Condition::PhraseKeyword("ＡＡＡ１０".into()))),
                        ]
                    ),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_two_keywords() {
            let target = Query::new("ＡＡＡ ＢＢＢ".into());
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
        fn test_query_to_condition_two_phrase_keywords() {
            let target = Query::new("\"ＡＡＡ ＢＢＢ\" \"ＣＣＣ ＤＤＤ\"".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::PhraseKeyword("ＡＡＡ ＢＢＢ".into()),
                            Condition::PhraseKeyword("ＣＣＣ ＤＤＤ".into())
                        ]
                    ),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_two_negative_keywords() {
            let target = Query::new("-ＡＡＡ -ＢＢＢ".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Not(Box::new(Condition::Keyword("ＡＡＡ".into()))),
                            Condition::Not(Box::new(Condition::Keyword("ＢＢＢ".into())))
                        ]
                    ),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_two_negative_phrase_keywords() {
            let target = Query::new("-\"ＡＡＡ ＢＢＢ\" -\"ＣＣＣ ＤＤＤ\"".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Not(Box::new(Condition::PhraseKeyword(
                                "ＡＡＡ ＢＢＢ".into()
                            ))),
                            Condition::Not(Box::new(Condition::PhraseKeyword(
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
            let target = Query::new("ＡＡＡ \"ＢＢＢ\" -\"ＣＣＣ\" -ＤＤＤ".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Keyword("ＡＡＡ".into()),
                            Condition::PhraseKeyword("ＢＢＢ".into()),
                            Condition::Not(Box::new(Condition::PhraseKeyword("ＣＣＣ".into()))),
                            Condition::Not(Box::new(Condition::Keyword("ＤＤＤ".into())))
                        ]
                    ),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_multi_keywords_without_space() {
            let target =
                Query::new("ＡＡＡ\"ＢＢＢ\"\"ｂｂｂ\"-\"ＣＣＣ\"-\"ｃｃｃ\"-ＤＤＤ".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Keyword("ＡＡＡ".into()),
                            Condition::PhraseKeyword("ＢＢＢ".into()),
                            Condition::PhraseKeyword("ｂｂｂ".into()),
                            Condition::Not(Box::new(Condition::PhraseKeyword("ＣＣＣ".into()))),
                            Condition::Not(Box::new(Condition::PhraseKeyword("ｃｃｃ".into()))),
                            Condition::Not(Box::new(Condition::Keyword("ＤＤＤ".into())))
                        ]
                    ),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_two_keywords_with_or() {
            let target = Query::new("ＡＡＡ or ＢＢＢ".into());
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
        fn test_query_to_condition_two_phrase_keywords_with_or() {
            let target = Query::new("\"ＡＡＡ ＢＢＢ\" or \"ＣＣＣ ＤＤＤ\"".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Operator(
                        Operator::Or,
                        vec![
                            Condition::PhraseKeyword("ＡＡＡ ＢＢＢ".into()),
                            Condition::PhraseKeyword("ＣＣＣ ＤＤＤ".into())
                        ]
                    ),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_two_negative_keywords_with_or() {
            let target = Query::new("-ＡＡＡ or -ＢＢＢ".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Operator(
                        Operator::Or,
                        vec![
                            Condition::Not(Box::new(Condition::Keyword("ＡＡＡ".into()))),
                            Condition::Not(Box::new(Condition::Keyword("ＢＢＢ".into())))
                        ]
                    ),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_two_negative_phrase_keywords_with_or() {
            let target = Query::new("-\"ＡＡＡ ＢＢＢ\" or -\"ＣＣＣ ＤＤＤ\"".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Operator(
                        Operator::Or,
                        vec![
                            Condition::Not(Box::new(Condition::PhraseKeyword(
                                "ＡＡＡ ＢＢＢ".into()
                            ))),
                            Condition::Not(Box::new(Condition::PhraseKeyword(
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
            let target = Query::new("ＡＡＡ or or ＢＢＢ".into());
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
            let target = Query::new("ＡＡＡ and ＢＢＢ".into());
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
        fn test_query_to_condition_two_phrase_keywords_with_and() {
            let target = Query::new("\"ＡＡＡ ＢＢＢ\" and \"ＣＣＣ ＤＤＤ\"".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::PhraseKeyword("ＡＡＡ ＢＢＢ".into()),
                            Condition::PhraseKeyword("ＣＣＣ ＤＤＤ".into())
                        ]
                    ),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_two_negative_keywords_with_and() {
            let target = Query::new("-ＡＡＡ and -ＢＢＢ".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Not(Box::new(Condition::Keyword("ＡＡＡ".into()))),
                            Condition::Not(Box::new(Condition::Keyword("ＢＢＢ".into())))
                        ]
                    ),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_two_negative_phrase_keywords_with_and() {
            let target = Query::new("-\"ＡＡＡ ＢＢＢ\" and -\"ＣＣＣ ＤＤＤ\"".into());
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Not(Box::new(Condition::PhraseKeyword(
                                "ＡＡＡ ＢＢＢ".into()
                            ))),
                            Condition::Not(Box::new(Condition::PhraseKeyword(
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
            let target = Query::new("ＡＡＡ and and ＢＢＢ".into());
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
                "ＡＡＡ and ＢＢＢ or ＣＣＣ ＤＤＤ and ＥＥＥ or ＦＦＦ or ＧＧＧ ＨＨＨ".into(),
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
            let target = Query::new("ＡＡＡ and or and or ＢＢＢ".into());
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
        fn test_query_to_condition_and_or_in_phrase_keyword() {
            let target = Query::new(
                "ＡＡＡ \" and ＢＢＢ or ＣＣＣ and \" \" or ＤＤＤ and ＥＥＥ or \" ＦＦＦ".into(),
            );
            let actual = target.to_condition().unwrap();
            assert_eq!(
                actual,
                (
                    false,
                    Condition::Operator(
                        Operator::And,
                        vec![
                            Condition::Keyword("ＡＡＡ".into()),
                            Condition::PhraseKeyword(" and ＢＢＢ or ＣＣＣ and ".into()),
                            Condition::PhraseKeyword(" or ＤＤＤ and ＥＥＥ or ".into()),
                            Condition::Keyword("ＦＦＦ".into()),
                        ]
                    ),
                    false
                )
            )
        }

        #[test]
        fn test_query_to_condition_full_pattern() {
            let target = Query::new(" ＡＡＡ  Ａｎｄ -ＢＢＢ ＡnＤ ＣorＣ  ｃｃｃ Ｏr  \"c1 and c2\"  -\"c3 or c4\"  ＤandＤ anD \" Ｐ１ and Ｐ２ -(Ｐ３ or Ｐ４) \"  ａnｄ  -\" ＮＰ１ and ＮＰ２ -(ＮＰ３ or ＮＰ４) \"  oＲ  ＩＩＩ and ".into());
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
                                    Condition::Not(Box::new(Condition::Keyword("ＢＢＢ".into()))),
                                    Condition::Keyword("ＣorＣ".into()),
                                    Condition::Keyword("ｃｃｃ".into()),
                                ]
                            ),
                            Condition::Operator(
                                Operator::And,
                                vec![
                                    Condition::PhraseKeyword("c1 and c2".into()),
                                    Condition::Not(Box::new(Condition::PhraseKeyword(
                                        "c3 or c4".into()
                                    ))),
                                    Condition::Keyword("ＤandＤ".into()),
                                    Condition::PhraseKeyword(
                                        " Ｐ１ and Ｐ２ -(Ｐ３ or Ｐ４) ".into()
                                    ),
                                    Condition::Not(Box::new(Condition::PhraseKeyword(
                                        " ＮＰ１ and ＮＰ２ -(ＮＰ３ or ＮＰ４) ".into()
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
            let target = Query::new("and ＡＡＡ ＢＢＢ and".into());
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
            let target = Query::new(" and ＡＡＡ ＢＢＢ and ".into());
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
            let target = Query::new("or ＡＡＡ ＢＢＢ or".into());
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
            let target = Query::new(" or ＡＡＡ ＢＢＢ or ".into());
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
}
