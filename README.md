# search-query-parser

[![crates.io](https://img.shields.io/crates/v/search-query-parser.svg)](https://crates.io/crates/search-query-parser)
[![docs.rs](https://docs.rs/search-query-parser/badge.svg)](https://docs.rs/search-query-parser)
[![build](https://github.com/dimmy82/search-query-parser/actions/workflows/build_and_test.yml/badge.svg)](https://github.com/dimmy82/search-query-parser/actions)

search-query-parser is made to parse complex search query into layered search conditions, so it will be easy to construct Elasticsearch query DSL or something else.

the complex search query like this: ↓↓↓

`(word１ and -word２) or (("phrase word １" or -"phrase word ２") and -(" a long phrase word " or word３))`

will be parsed into layered search conditions like this: ↓↓↓

```Rust
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
```
