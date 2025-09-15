mod tokens; // This tells Rust to include src/tokens.rs

use std::{collections::HashMap, fs};
use tokens::{Token, TokenType};

fn main() {
    // let input = "user : Map<String, String> = {\"name\": \"Alice\", \"role\": \"admin\"} ";
    let input = fs::read_to_string("./syntax.mylang").unwrap();
    // println!("Source code:\n{}", input);

    let chars: Vec<char> = input.chars().collect();
    let mut tokens: Vec<Token> = Vec::new();

    // Keyword map
    let mut keywords: HashMap<&str, TokenType> = HashMap::new();
    keywords.insert("let", TokenType::Let);
    keywords.insert("fn", TokenType::Function);
    keywords.insert("import", TokenType::Import);
    keywords.insert("struct", TokenType::Struct);
    keywords.insert("enum", TokenType::Enum);
    keywords.insert("map", TokenType::Map);
    keywords.insert("true", TokenType::Boolean);
    keywords.insert("false", TokenType::Boolean);

    // Operators map
    let mut operators: HashMap<&str, TokenType> = HashMap::new();
    operators.insert("=", TokenType::Eq);
    operators.insert("+", TokenType::Plus);
    operators.insert("-", TokenType::Minus);
    operators.insert("*", TokenType::Star);
    operators.insert("/", TokenType::Slash);
    operators.insert("%", TokenType::Percent);
    operators.insert("!", TokenType::Bang);
    operators.insert("<", TokenType::Lt);
    operators.insert(">", TokenType::Gt);

    operators.insert("(", TokenType::OpenParen);
    operators.insert(")", TokenType::CloseParen);
    operators.insert("{", TokenType::OpenBrace);
    operators.insert("}", TokenType::CloseBrace);
    operators.insert("[", TokenType::OpenBracket);
    operators.insert("]", TokenType::CloseBracket);

    operators.insert(",", TokenType::Comma);
    operators.insert(";", TokenType::Semi);
    operators.insert(".", TokenType::Dot);
    operators.insert(":", TokenType::Colon);
    operators.insert("@", TokenType::At);
    operators.insert("#", TokenType::Pound);
    operators.insert("~", TokenType::Tilde);
    operators.insert("?", TokenType::Question);
    operators.insert("$", TokenType::Dollar);
    operators.insert("&", TokenType::And);
    operators.insert("|", TokenType::Or);
    operators.insert("_", TokenType::Underscore);

    // Multi-character operators can also be added

    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];

        // Skip whitespace
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // For value inside string literal
        // Ex: "hello world"
        if c == '"' {
            let start = i + 1; // skip opening "
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            let value: &str = &input[start..i];
            tokens.push(Token {
                kind: TokenType::String,
                value,
            });
            i += 1; // skip closing "
            continue;
        }

        // Numbers
        if c.is_digit(10) {
            let start = i;
            while i < chars.len() && chars[i].is_digit(10) {
                i += 1;
            }
            let value: &str = &input[start..i];
            tokens.push(Token {
                kind: TokenType::Number,
                value,
            });
            continue;
        }

        // Alphabetic: keywords or identifiers
        if c.is_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_alphanumeric() {
                i += 1;
            }
            let word: &str = &input[start..i];
            let kind = keywords.get(word).unwrap_or(&TokenType::Identifier);
            tokens.push(Token {
                kind: *kind,
                value: word,
            });
            continue;
        }

        // Operators (single or multi-character)
        let start = i;
        let mut matched = false;
        for len in (1..=3).rev() {
            // check for operators up to length 3
            if i + len <= chars.len() {
                let op = &input[start..start + len];
                if let Some(kind) = operators.get(op) {
                    tokens.push(Token {
                        kind: *kind,
                        value: op,
                    });
                    i += len;
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            continue;
        }

        // Unknown character: skip
        i += 1;
    }

    for token in &tokens {
        println!("{:?}", token);
    }
}
