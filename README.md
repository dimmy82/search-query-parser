# search-query-parser

[![crates.io](https://img.shields.io/crates/v/search-query-parser.svg)](https://crates.io/crates/search-query-parser)
[![docs.rs](https://docs.rs/search-query-parser/badge.svg)](https://docs.rs/search-query-parser)
[![build](https://github.com/dimmy82/search-query-parser/actions/workflows/build_and_test.yml/badge.svg)](https://github.com/dimmy82/search-query-parser/actions)

## what is this library for

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

the conditions are constructed by the `enum Condition` and `enum Operator`.

```Rust
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Condition {
    None,
    Keyword(String),
    PhraseKeyword(String),
    Not(Box<Condition>),
    Operator(Operator, Vec<Condition>),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Operator {
    And,
    Or,
}
```

## usage

```toml
[dependencies]
search-query-parser = "0.1.1"
```

```Rust
use search_query_parser::parse_query_to_condition;

let condition = parse_query_to_condition("any query string you like")?;
```

## parse rules

### 1. space {\u0020} or full width space {\u3000} are identified as `AND` operator

```Rust
assert_eq!(
    parse_query_to_condition("word1 word2").unwrap(),
    parse_query_to_condition("word1 AND word2").unwrap()
);
```

### 2. conditions in brackets have higher priority

```Rust
assert_eq!(
    parse_query_to_condition("word1 OR (word2 AND word3)").unwrap(),
    Condition::Operator(
        Operator::Or,
        vec![
            Condition::Keyword("word1".into()),
            Condition::Operator(
                Operator::And,
                vec![
                    Condition::Keyword("word2".into()),
                    Condition::Keyword("word3".into()),
                ]
            )
        ]
    )
);
```

### 3. `AND` operator has higher priority than `OR` operator

```Rust
assert_eq!(
    parse_query_to_condition("word1 OR word2 AND word3").unwrap(),
    parse_query_to_condition("word1 OR (word2 AND word3)").unwrap()
);
```

### To Be Continued ......
