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

### 1. for Rust project

```toml
[dependencies]
search-query-parser = "0.1.4"
```

```Rust
use search_query_parser::parse_query_to_condition;

let condition = parse_query_to_condition("any query string you like")?;
```

### 2. for REST Api

[refer to search-query-parser-api repository](https://github.com/dimmy82/search-query-parser-api)

### 3. for JVM language via JNI

[refer to search-query-parser-cdylib repository](https://github.com/dimmy82/search-query-parser-cdylib)

## parse rules

### 1. space {\u0020} or full width space {\u3000} are identified as `AND` operator

```Rust
fn test_keywords_concat_with_spaces() {
    let actual = parse_query_to_condition("word1 word2").unwrap();
    assert_eq!(
        actual,
        Condition::Operator(
            Operator::And,
            vec![
                Condition::Keyword("word1".into()),
                Condition::Keyword("word2".into())
            ]
        )
    )
}
```

### 2. `AND` operator has higher priority than `OR` operator

```Rust
fn test_keywords_concat_with_and_or() {
    let actual =
        parse_query_to_condition("word1 OR word2 AND word3").unwrap();
    assert_eq!(
        actual,
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
    )
}
```

### 3. conditions in brackets have higher priority

```Rust
fn test_brackets() {
    let actual =
        parse_query_to_condition("word1 AND (word2 OR word3)")
            .unwrap();
    assert_eq!(
        actual,
        Condition::Operator(
            Operator::And,
            vec![
                Condition::Keyword("word1".into()),
                Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("word2".into()),
                        Condition::Keyword("word3".into()),
                    ]
                )
            ]
        )
    )
}
```

### 4. double quote will be parsed for phrase keyword

```Rust
fn test_double_quote() {
    let actual = parse_query_to_condition(
        "\"word1 AND (word2 OR word3)\" word4",
    )
    .unwrap();
    assert_eq!(
        actual,
        Condition::Operator(
            Operator::And,
            vec![
                Condition::PhraseKeyword(
                    "word1 AND (word2 OR word3)".into()
                ),
                Condition::Keyword("word4".into()),
            ]
        )
    )
}
```

### 5. minus(hyphen) will be parsed for negative condition
※ it can be used before keyword, phrase keyword or brackets

```Rust
fn test_minus() {
    let actual = parse_query_to_condition(
        "-word1 -\"word2\" -(word3 OR word4)",
    )
    .unwrap();
    assert_eq!(
        actual,
        Condition::Operator(
            Operator::And,
            vec![
                Condition::Not(Box::new(Condition::Keyword("word1".into()))),
                Condition::Not(Box::new(Condition::PhraseKeyword("word2".into()))),
                Condition::Not(Box::new(Condition::Operator(
                    Operator::Or,
                    vec![
                        Condition::Keyword("word3".into()),
                        Condition::Keyword("word4".into())
                    ]
                ))),
            ]
        )
    )
}
```

### 6. correcting incorrect search query
1. empty brackets
```Rust
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
```

2. reversed brackets
```Rust
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
```

3. wrong number of brackets
```Rust
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
```

4. empty phrase keyword
```Rust
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
```

5. wrong number or double quote
```Rust
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
```

6. and or are next to each other
```Rust
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
```

### 7. search query optimization
```Rust
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
```
