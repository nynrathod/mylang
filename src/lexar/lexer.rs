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
    operators.insert("#", TokenType::Pound);
    operators.insert("~", TokenType::Tilde);
    operators.insert("?", TokenType::Question);
    operators.insert("$", TokenType::Dollar);

    // Special identifier
    operators.insert("_", TokenType::Underscore);

    let mut i = 0;
    let mut line: usize = 1;
    let mut col: usize = 1;
    while i < chars.len() {
        let c = chars[i];

        // Skip whitespace
        if c.is_whitespace() {
            if c == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
            i += 1;
            continue;
        }

        // Skip comments starting with // until newline
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            i += 2; // skip the `//`
            col += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
                col += 1;
            }
            continue;
        }

        // Skip C-style multiline comments /* ... */
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            col += 2;
            // Find closing */
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                if chars[i] == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
                i += 1;
            }
            if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '/' {
                i += 2;
                col += 2;
            }
            continue;
        }

        // Multi-character operators first
        // Always check for ..= and .. before handling numbers/floats
        if i + 3 <= chars.len() {
            let op: String = chars[i..i + 3].iter().collect();
            if op == "..=" {
                tokens.push(Token {
                    kind: TokenType::RangeInc, // inclusive
                    value: Box::leak(op.into_boxed_str()),
                    line,
                    col,
                });
                i += 3;
                col += 3;
                continue;
            }
        }
        if i + 2 <= chars.len() {
            let op: String = chars[i..i + 2].iter().collect();
            if op == ".." {
                tokens.push(Token {
                    kind: TokenType::RangeExc, // exclusive
                    value: Box::leak(op.into_boxed_str()),
                    line,
                    col,
                });
                i += 2;
                col += 2;
                continue;
            }
        }

        // For value inside string literal
        // Ex: "hello world"
        if c == '"' {
            let token_line = line;
            let token_col = col;
            let start = i + 1; // skip opening "
            i += 1;
            col += 1;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
                col += 1;
            }
            // Only emit String token if closing quote is found
            if i < chars.len() && chars[i] == '"' {
                let value: String = chars[start..i].iter().collect();
                tokens.push(Token {
                    kind: TokenType::String,
                    value: Box::leak(value.into_boxed_str()),
                    line: token_line,
                    col: token_col,
                });
                i += 1; // skip closing "
                col += 1;
            }
            // If no closing quote, skip emitting String token
            continue;
        }

        // Numbers and floats
        if c.is_digit(10) {
            let token_line = line;
            let token_col = col;
            let start = i;
            let mut has_dot = false;
            let mut has_exp = false;
            let mut exp_idx = 0;
            // Integer part
            while i < chars.len() && chars[i].is_digit(10) {
                i += 1;
                col += 1;
            }
            // Fractional part (float only if . is followed by digit and not .. or ..=)
            if i < chars.len() && chars[i] == '.' {
                // Check if this is a range operator, not a float
                if i + 1 < chars.len() && chars[i + 1] == '.' {
                    // Do not consume . here, let range logic above handle it
                } else if i + 1 < chars.len() && chars[i + 1].is_digit(10) {
                    // Only treat as float if there is at least one digit after the dot
                    has_dot = true;
                    i += 1;
                    col += 1;
                    while i < chars.len() && chars[i].is_digit(10) {
                        i += 1;
                        col += 1;
                    }
                }
                // else: do not consume the dot, let it be tokenized as a Dot later
            }
            // Exponent part
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                has_exp = true;
                exp_idx = i;
                i += 1;
                col += 1;
                if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                    i += 1;
                    col += 1;
                }
                let exp_start = i;
                while i < chars.len() && chars[i].is_digit(10) {
                    i += 1;
                    col += 1;
                }
                // If exponent is not followed by digits, treat as integer/float up to 'e'
                if exp_start == i {
                    i = exp_idx; // rewind to before 'e'
                    col -= i - exp_idx;
                    has_exp = false;
                }
            }
            let value: String = chars[start..i].iter().collect();
            tokens.push(Token {
                kind: if has_dot || has_exp {
                    TokenType::Float
                } else {
                    TokenType::Number
                },
                value: Box::leak(value.into_boxed_str()),
                line: token_line,
                col: token_col,
            });
            continue;
        }

        // Alphabetic: keywords or identifiers
        if c.is_alphabetic() || c == '_' {
            let token_line = line;
            let token_col = col;
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
                col += 1;
            }
            // Use char indices for slicing to support unicode
            let word: String = chars[start..i].iter().collect();
            let kind = keywords
                .get(word.as_str())
                .unwrap_or(&TokenType::Identifier);

            // Disallow identifiers starting with underscore
            if word.contains('_') {
                tokens.push(Token {
                    kind: TokenType::Unknown,
                    value: Box::leak(word.clone().into_boxed_str()),
                    line: token_line,
                    col: token_col,
                });
                // Optionally: emit error here or let parser/analyzer handle centralized error
            } else {
                tokens.push(Token {
                    kind: *kind,
                    value: Box::leak(word.into_boxed_str()),
                    line: token_line,
                    col: token_col,
                });
            }
            continue;
        }

        // Operators (single or multi-character)
        let token_line = line;
        let token_col = col;
        let start = i;
        let mut matched = false;
        for len in (1..=3).rev() {
            // check for operators up to length 3
            if i + len <= chars.len() {
                let op: String = chars[start..start + len].iter().collect();
                if let Some(kind) = operators.get(op.as_str()) {
                    tokens.push(Token {
                        kind: *kind,
                        value: Box::leak(op.into_boxed_str()),
                        line: token_line,
                        col: token_col,
                    });
                    i += len;
                    col += len;
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            continue;
        }

        // Unknown character: emit Unknown token
        let value: String = chars[i..i + 1].iter().collect();
        tokens.push(Token {
            kind: TokenType::Unknown,
            value: Box::leak(value.into_boxed_str()),
            line,
            col,
        });
        i += 1;
        col += 1;
    }

    return tokens;
}
