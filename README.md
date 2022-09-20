# search-query-parser

[![crates.io](https://img.shields.io/crates/v/search-query-parser.svg)](https://crates.io/crates/search-query-parser)
[![docs.rs](https://docs.rs/search-query-parser/badge.svg)](https://docs.rs/search-query-parser)
[![build](https://github.com/dimmy82/search-query-parser/actions/workflows/build_and_test.yml/badge.svg)](https://github.com/dimmy82/search-query-parser/actions)

search-query-parser is made to parse complex search query into layered search conditions.

the complex search query like this: ↓↓↓

`(keyword１ and -keyword２) or (("phrase keyword １" or -"phrase keyword ２") and -(" a long phrase keyword " or keyword３))`

will be parsed into layered search conditions like this: ↓↓↓

```Rust
Condition::Operator(
    Operator::Or,
    vec![
        Condition::Operator(
            Operator::And,
            vec![
                Condition::Keyword("keyword１".into()),
                Condition::Not(Box::new(Condition::Keyword("keyword２".into()))),
            ]
        ),
        Condition::Operator(
            Operator::And,
            vec![
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::PhraseKeyword("phrase keyword １".into()),
                        Condition::Not(Box::new(Condition::PhraseKeyword(
                            "phrase keyword ２".into()
                        )))
                    ]
                ),
                Condition::Not(Box::new(Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::PhraseKeyword(" a long phrase keyword ".into()),
                        Condition::Keyword("keyword３".into())
                    ]
                )))
            ]
        ),
    ]
)
```
