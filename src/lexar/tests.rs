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
    fn test_operators() {
        let input = "+ - * / == != < > <= >=";
        let tokens = lex(input);
        assert_eq!(tokens[0].kind, TokenType::Plus);
        assert_eq!(tokens[1].kind, TokenType::Minus);
        assert_eq!(tokens[2].kind, TokenType::Star);
        assert_eq!(tokens[3].kind, TokenType::Slash);
        assert_eq!(tokens[4].kind, TokenType::EqEq);
        assert_eq!(tokens[5].kind, TokenType::NotEq);
        assert_eq!(tokens[6].kind, TokenType::Lt);
        assert_eq!(tokens[7].kind, TokenType::Gt);
        assert_eq!(tokens[8].kind, TokenType::LtEq);
        assert_eq!(tokens[9].kind, TokenType::GtEq);
    }

    #[test]
    fn test_keywords() {
        let input = "fn if else for in return struct enum import";
        let tokens = lex(input);
        assert_eq!(tokens[0].kind, TokenType::Function);
        assert_eq!(tokens[1].kind, TokenType::If);
        assert_eq!(tokens[2].kind, TokenType::Else);
        assert_eq!(tokens[3].kind, TokenType::For);
        assert_eq!(tokens[4].kind, TokenType::In);
        assert_eq!(tokens[5].kind, TokenType::Return);
        assert_eq!(tokens[6].kind, TokenType::Struct);
        assert_eq!(tokens[7].kind, TokenType::Enum);
        assert_eq!(tokens[8].kind, TokenType::Import);
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

    // #[test]
    // fn test_invalid_token() {
    //     let input = "@";
    //     let tokens = lex(input);
    //     // Should produce an Unknown token or similar for invalid character
    //     let has_unknown = tokens.iter().any(|t| matches!(t.kind, TokenType::Unknown));
    //     assert!(
    //         has_unknown,
    //         "Lexer should produce Unknown token for invalid input"
    //     );
    // }

    // #[test]
    // fn test_unterminated_string() {
    //     let input = "\"unterminated string";
    //     let tokens = lex(input);
    //     // Should produce an Unknown token or similar for unterminated string
    //     let has_unknown = tokens.iter().any(|t| matches!(t.kind, TokenType::Unknown));
    //     assert!(
    //         has_unknown,
    //         "Lexer should produce Unknown token for unterminated string"
    //     );
    // }

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

    // #[test]
    // fn test_comments_ignored() {
    //     let input = "let x = 42; // this is a comment\nlet y = 10;";
    //     let tokens = lex(input);
    //     // Comments should be filtered out
    //     let has_comment = tokens.iter().any(|t| matches!(t.kind, TokenType::Comment));
    //     assert!(!has_comment);
    // }

    #[test]
    fn test_type_annotations() {
        let input = "let x: Int = 42; let s: Str = \"hi\"; let b: Bool = true;";
        let tokens = lex(input);
        assert_eq!(tokens[2].kind, TokenType::Colon);
        assert_eq!(tokens[3].kind, TokenType::Identifier);
        assert_eq!(tokens[3].value, "Int");
    }
}
