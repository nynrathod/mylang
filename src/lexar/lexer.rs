use crate::lexar::token::{Token, TokenType};
use std::collections::HashMap;

pub fn lex(input: &str) -> Vec<Token<'_>> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens: Vec<Token> = Vec::new();

    // --- Keyword Maps ---
    let mut keywords: HashMap<&str, TokenType> = HashMap::new();

    // Declarations

    keywords.insert("let", TokenType::Let);
    keywords.insert("mut", TokenType::Mut);
    keywords.insert("fn", TokenType::Function);
    keywords.insert("import", TokenType::Import);
    keywords.insert("struct", TokenType::Struct);
    keywords.insert("enum", TokenType::Enum);
    keywords.insert("map", TokenType::Map);

    // Control flow statements
    keywords.insert("if", TokenType::If);
    keywords.insert("else", TokenType::Else);
    keywords.insert("for", TokenType::For);
    keywords.insert("in", TokenType::In);

    // Statement keywords
    keywords.insert("return", TokenType::Return);
    keywords.insert("break", TokenType::Break);
    keywords.insert("continue", TokenType::Continue);
    keywords.insert("print", TokenType::Print);

    // Special values and types
    keywords.insert("Some", TokenType::Some);
    keywords.insert("true", TokenType::Boolean);
    keywords.insert("false", TokenType::Boolean);

    // --- Operator and Punctuation Map ---
    let mut operators: HashMap<&str, TokenType> = HashMap::new();

    // Assignment and arithmetic operators
    operators.insert("=", TokenType::Eq);
    operators.insert("+", TokenType::Plus);
    operators.insert("-", TokenType::Minus);
    operators.insert("*", TokenType::Star);
    operators.insert("/", TokenType::Slash);
    operators.insert("%", TokenType::Percent);

    // Logical and comparison operators
    operators.insert("!", TokenType::Bang);
    operators.insert("<", TokenType::Lt);
    operators.insert(">", TokenType::Gt);
    operators.insert("&", TokenType::And);
    operators.insert("|", TokenType::Or);

    operators.insert("==", TokenType::EqEq);
    operators.insert("===", TokenType::EqEqEq);
    operators.insert("!=", TokenType::NotEq);
    operators.insert("!==", TokenType::NotEqEq);
    operators.insert(">=", TokenType::GtEq);
    operators.insert("<=", TokenType::LtEq);
    operators.insert("&&", TokenType::AndAnd);
    operators.insert("||", TokenType::OrOr);

    // Compound assignment operators
    operators.insert("+=", TokenType::PlusEq);
    operators.insert("-=", TokenType::MinusEq);
    operators.insert("*=", TokenType::StarEq);
    operators.insert("/=", TokenType::SlashEq);
    operators.insert("%=", TokenType::PercentEq);

    // Arrow operators
    operators.insert("->", TokenType::Arrow);
    operators.insert("=>", TokenType::FatArrow);

    // Grouping and delimiter symbols
    operators.insert("(", TokenType::OpenParen);
    operators.insert(")", TokenType::CloseParen);
    operators.insert("{", TokenType::OpenBrace);
    operators.insert("}", TokenType::CloseBrace);
    operators.insert("[", TokenType::OpenBracket);
    operators.insert("]", TokenType::CloseBracket);

    // Punctuation
    operators.insert(",", TokenType::Comma);
    operators.insert(";", TokenType::Semi);
    operators.insert(".", TokenType::Dot);
    operators.insert("..=", TokenType::RangeInc);
    operators.insert("..", TokenType::RangeExc);

    // Miscellaneous symbols
    operators.insert(":", TokenType::Colon);
    operators.insert("@", TokenType::At);
    operators.insert("#", TokenType::Pound);
    operators.insert("~", TokenType::Tilde);
    operators.insert("?", TokenType::Question);
    operators.insert("$", TokenType::Dollar);

    // Special identifier
    operators.insert("_", TokenType::Underscore);

    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];

        // Skip whitespace
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // Skip comments starting with // untill new line
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            // Skip until newline
            i += 2; // skip the `//`
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Multi-character operators first
        if i + 3 <= chars.len() && &input[i..i + 3] == "..=" {
            tokens.push(Token {
                kind: TokenType::RangeInc, // inclusive
                value: "..=",
            });
            i += 3;
            continue;
        } else if i + 2 <= chars.len() && &input[i..i + 2] == ".." {
            tokens.push(Token {
                kind: TokenType::RangeExc, // exclusive
                value: "..",
            });
            i += 2;
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

        // Numbers (Not supporting float value)
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

    return tokens;
}
