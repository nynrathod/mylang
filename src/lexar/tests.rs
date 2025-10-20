// --- VALID TESTS ---
#[cfg(test)]
mod lexer_tests {
    use crate::lexar::lexer::lex;
    use crate::lexar::token::TokenType;

    #[test]
    fn test_basic_tokens() {
        let input = "let x = 42;";
        let tokens = lex(input);
        assert_eq!(tokens[0].kind, TokenType::Let);
        assert_eq!(tokens[1].kind, TokenType::Identifier);
        assert_eq!(tokens[1].value, "x");
        assert_eq!(tokens[2].kind, TokenType::Eq);
        assert_eq!(tokens[3].kind, TokenType::Number);
        assert_eq!(tokens[3].value, "42");
        assert_eq!(tokens[4].kind, TokenType::Semi);
    }

    #[test]
    fn test_string_literals() {
        let input = r#"let s = "hello world";"#;
        let tokens = lex(input);
        assert_eq!(tokens[3].kind, TokenType::String);
        assert_eq!(tokens[3].value, "hello world");
    }

    #[test]
    fn test_boolean_literals() {
        let input = "let a = true; let b = false;";
        let tokens = lex(input);
        assert_eq!(tokens[3].kind, TokenType::Boolean);
        assert_eq!(tokens[3].value, "true");
        assert_eq!(tokens[8].kind, TokenType::Boolean);
        assert_eq!(tokens[8].value, "false");
    }

    #[test]
    fn test_arithmetic_operators() {
        let input = "+ - * / %";
        let tokens = lex(input);
        assert_eq!(tokens[0].kind, TokenType::Plus);
        assert_eq!(tokens[1].kind, TokenType::Minus);
        assert_eq!(tokens[2].kind, TokenType::Star);
        assert_eq!(tokens[3].kind, TokenType::Slash);
        assert_eq!(tokens[4].kind, TokenType::Percent);
    }

    #[test]
    fn test_assignment_operators() {
        let input = "= += -= *= /= %=";
        let tokens = lex(input);
        assert_eq!(tokens[0].kind, TokenType::Eq);
        assert_eq!(tokens[1].kind, TokenType::PlusEq);
        assert_eq!(tokens[2].kind, TokenType::MinusEq);
        assert_eq!(tokens[3].kind, TokenType::StarEq);
        assert_eq!(tokens[4].kind, TokenType::SlashEq);
        assert_eq!(tokens[5].kind, TokenType::PercentEq);
    }

    #[test]
    fn test_comparison_operators() {
        let input = "== === != !== > < >= <=";
        let tokens = lex(input);
        assert_eq!(tokens[0].kind, TokenType::EqEq);
        assert_eq!(tokens[1].kind, TokenType::EqEqEq);
        assert_eq!(tokens[2].kind, TokenType::NotEq);
        assert_eq!(tokens[3].kind, TokenType::NotEqEq);
        assert_eq!(tokens[4].kind, TokenType::Gt);
        assert_eq!(tokens[5].kind, TokenType::Lt);
        assert_eq!(tokens[6].kind, TokenType::GtEq);
        assert_eq!(tokens[7].kind, TokenType::LtEq);
    }

    #[test]
    fn test_logical_operators() {
        let input = "! & | && ||";
        let tokens = lex(input);
        assert_eq!(tokens[0].kind, TokenType::Bang);
        assert_eq!(tokens[1].kind, TokenType::And);
        assert_eq!(tokens[2].kind, TokenType::Or);
        assert_eq!(tokens[3].kind, TokenType::AndAnd);
        assert_eq!(tokens[4].kind, TokenType::OrOr);
    }

    #[test]
    fn test_arrow_operators() {
        let input = "-> =>";
        let tokens = lex(input);
        assert_eq!(tokens[0].kind, TokenType::Arrow);
        assert_eq!(tokens[1].kind, TokenType::FatArrow);
    }

    #[test]
    fn test_delimiters_and_punctuation() {
        let input = "( ) { } [ ] , ; . .. ..= : # ~ ? $ _";
        let tokens = lex(input);
        assert_eq!(tokens[0].kind, TokenType::OpenParen);
        assert_eq!(tokens[1].kind, TokenType::CloseParen);
        assert_eq!(tokens[2].kind, TokenType::OpenBrace);
        assert_eq!(tokens[3].kind, TokenType::CloseBrace);
        assert_eq!(tokens[4].kind, TokenType::OpenBracket);
        assert_eq!(tokens[5].kind, TokenType::CloseBracket);
        assert_eq!(tokens[6].kind, TokenType::Comma);
        assert_eq!(tokens[7].kind, TokenType::Semi);
        assert_eq!(tokens[8].kind, TokenType::Dot);
        assert_eq!(tokens[9].kind, TokenType::RangeExc);
        assert_eq!(tokens[10].kind, TokenType::RangeInc);
        assert_eq!(tokens[11].kind, TokenType::Colon);
        assert_eq!(tokens[12].kind, TokenType::Pound);
        assert_eq!(tokens[13].kind, TokenType::Tilde);
        assert_eq!(tokens[14].kind, TokenType::Question);
        assert_eq!(tokens[15].kind, TokenType::Dollar);
        assert_eq!(tokens[16].kind, TokenType::Underscore);
    }

    #[test]
    fn test_keywords() {
        let input = "let mut fn if else for in return break continue struct enum import print";
        let tokens = lex(input);
        assert_eq!(tokens[0].kind, TokenType::Let);
        assert_eq!(tokens[1].kind, TokenType::Mut);
        assert_eq!(tokens[2].kind, TokenType::Function);
        assert_eq!(tokens[3].kind, TokenType::If);
        assert_eq!(tokens[4].kind, TokenType::Else);
        assert_eq!(tokens[5].kind, TokenType::For);
        assert_eq!(tokens[6].kind, TokenType::In);
        assert_eq!(tokens[7].kind, TokenType::Return);
        assert_eq!(tokens[8].kind, TokenType::Break);
        assert_eq!(tokens[9].kind, TokenType::Continue);
        assert_eq!(tokens[10].kind, TokenType::Struct);
        assert_eq!(tokens[11].kind, TokenType::Enum);
        assert_eq!(tokens[12].kind, TokenType::Import);
        assert_eq!(tokens[13].kind, TokenType::Print);
    }

    #[test]
    fn test_array_literal() {
        let input = "[1, 2, 3]";
        let tokens = lex(input);
        assert_eq!(tokens[0].kind, TokenType::OpenBracket);
        assert_eq!(tokens[1].kind, TokenType::Number);
        assert_eq!(tokens[2].kind, TokenType::Comma);
        assert_eq!(tokens[3].kind, TokenType::Number);
        assert_eq!(tokens[4].kind, TokenType::Comma);
        assert_eq!(tokens[5].kind, TokenType::Number);
        assert_eq!(tokens[6].kind, TokenType::CloseBracket);
    }

    #[test]
    fn test_map_literal() {
        let input = r#"{"key": 42}"#;
        let tokens = lex(input);
        assert_eq!(tokens[0].kind, TokenType::OpenBrace);
        assert_eq!(tokens[1].kind, TokenType::String);
        assert_eq!(tokens[2].kind, TokenType::Colon);
        assert_eq!(tokens[3].kind, TokenType::Number);
        assert_eq!(tokens[4].kind, TokenType::CloseBrace);
    }

    #[test]
    fn test_function_declaration() {
        let input = "fn add(x: Int, y: Int) -> Int { return x + y; }";
        let tokens = lex(input);
        assert_eq!(tokens[0].kind, TokenType::Function);
        assert_eq!(tokens[1].kind, TokenType::Identifier);
        assert_eq!(tokens[1].value, "add");
        assert_eq!(tokens[2].kind, TokenType::OpenParen);
    }

    #[test]
    fn test_type_annotations() {
        let input = "let x: Int = 42; let s: Str = \"hi\"; let b: Bool = true;";
        let tokens = lex(input);
        assert_eq!(tokens[2].kind, TokenType::Colon);
        assert_eq!(tokens[3].kind, TokenType::Identifier);
        assert_eq!(tokens[3].value, "Int");
    }

    #[test]
    fn test_whitespace_only() {
        let input = "    \t\n  ";
        let tokens = lex(input);
        assert_eq!(
            tokens.len(),
            0,
            "Whitespace-only input should produce no tokens"
        );
    }

    #[test]
    fn test_empty_input() {
        let input = "";
        let tokens = lex(input);
        assert_eq!(tokens.len(), 0, "Empty input should produce no tokens");
    }

    #[test]
    fn test_long_identifier() {
        let input = "let thisIsAVeryLongIdentifierName123 = 1;";
        let tokens = lex(input);
        assert!(tokens
            .iter()
            .any(|t| t.value == "thisIsAVeryLongIdentifierName123"));
    }

    #[test]
    fn test_unicode_identifier() {
        let input = "let café = 1;";
        let tokens = lex(input);
        assert!(tokens.iter().any(|t| t.value == "café"));
    }

    #[test]
    fn test_multiple_semicolons() {
        let input = "let x = 1;;;;";
        let tokens = lex(input);
        let semi_count = tokens.iter().filter(|t| t.kind == TokenType::Semi).count();
        assert_eq!(semi_count, 4);
    }

    #[test]
    fn test_comments_ignored() {
        let input = "let x = 42; // this is a comment\nlet y = 10;";
        let tokens = lex(input);
        // Comments should be filtered out
        let expected_kinds = [
            TokenType::Let,
            TokenType::Identifier,
            TokenType::Eq,
            TokenType::Number,
            TokenType::Semi,
            TokenType::Let,
            TokenType::Identifier,
            TokenType::Eq,
            TokenType::Number,
            TokenType::Semi,
        ];
        assert_eq!(
            tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            expected_kinds
        );
    }

    // --- INVALID TESTS ---
    #[test]
    fn test_invalid_token() {
        let input = "@";
        let tokens = lex(input);
        // Should produce an Unknown token or similar for invalid character
        let has_unknown = tokens.iter().any(|t| matches!(t.kind, TokenType::Unknown));
        assert!(
            has_unknown,
            "Lexer should produce Unknown token for invalid input"
        );
    }

    #[test]
    fn test_invalid_number() {
        let input = "123abc";
        let tokens = lex(input);
        // Should produce a Number token followed by Identifier or Unknown
        let has_number = tokens.iter().any(|t| t.kind == TokenType::Number);
        let has_identifier = tokens.iter().any(|t| t.kind == TokenType::Identifier);
        assert!(has_number, "Lexer should produce Number token");
        assert!(
            has_identifier,
            "Lexer should produce Identifier token after invalid number"
        );
    }

    #[test]
    fn test_unterminated_string() {
        let input = "let s = \"unterminated;";
        let tokens = lex(input);
        // Should not produce a String token for unterminated string
        let has_string = tokens.iter().any(|t| t.kind == TokenType::String);
        assert!(
            !has_string,
            "Lexer should not produce String token for unterminated string"
        );
    }
}
