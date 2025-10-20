#[cfg(test)]
mod parser_tests {
    use crate::lexar::lexer::lex;
    use crate::parser::ast::AstNode;
    use crate::parser::Parser;

    // --- VALID TESTS ---
    #[test]
    fn test_variable_declaration() {
        let input = "let x: Int = 42;";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
        match result.unwrap() {
            AstNode::LetDecl { .. } => (),
            _ => panic!("Expected LetDecl"),
        }
    }

    #[test]
    fn test_mutable_variable() {
        let input = "let mut x = 10;";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
        match result.unwrap() {
            AstNode::LetDecl { mutable, .. } => assert!(mutable),
            _ => panic!("Expected mutable LetDecl"),
        }
    }

    #[test]
    fn test_function_declaration() {
        let input = "fn add(x: Int, y: Int) -> Int { return x + y; }";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
        match result.unwrap() {
            AstNode::FunctionDecl { name, params, .. } => {
                assert_eq!(name, "add");
                assert_eq!(params.len(), 2);
            }
            _ => panic!("Expected FunctionDecl"),
        }
    }

    #[test]
    fn test_if_statement() {
        let input = "if x > 5 { print(x); }";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
        match result.unwrap() {
            AstNode::ConditionalStmt { .. } => (),
            _ => panic!("Expected ConditionalStmt"),
        }
    }

    #[test]
    fn test_if_else_statement() {
        let input = "if x > 5 { print(x); } else { print(0); }";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
        match result.unwrap() {
            AstNode::ConditionalStmt { else_branch, .. } => assert!(else_branch.is_some()),
            _ => panic!("Expected ConditionalStmt with else_branch"),
        }
    }

    #[test]
    fn test_for_range_loop() {
        let input = "for i in 0..10 { print(i); }";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
        match result.unwrap() {
            AstNode::ForLoopStmt { .. } => (),
            _ => panic!("Expected ForLoopStmt"),
        }
    }

    #[test]
    fn test_for_array_loop() {
        let input = "for item in arr { print(item); }";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
    }

    #[test]
    fn test_array_literal() {
        let input = "let arr = [1, 2, 3];";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
    }

    #[test]
    fn test_map_literal() {
        let input = r#"let m = {"key": 42};"#;
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
    }

    #[test]
    fn test_function_call() {
        let input = "print(42);";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
    }

    #[test]
    fn test_binary_expression() {
        let input = "let x = 5 + 3 * 2;";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
    }

    #[test]
    fn test_comparison_expression() {
        let input = "let b = x > 5;";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
    }

    #[test]
    fn test_import_statement() {
        let input = "import http::Client::Fetchuser;";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
    }

    #[test]
    fn test_return_statement() {
        let input = "return 42;";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
        match result.unwrap() {
            AstNode::Return { .. } => (),
            _ => panic!("Expected Return statement"),
        }
    }

    #[test]
    fn test_break_statement() {
        let input = "break;";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
    }

    #[test]
    fn test_continue_statement() {
        let input = "continue;";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_ok());
    }

    #[test]
    fn test_full_program() {
        let input = r#"
            fn main() {
                let x = 42;
                print(x);
            }
        "#;
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_program();
        assert!(result.is_ok());
    }

    // --- INVALID TESTS ---
    #[test]
    fn test_invalid_missing_variable_name() {
        let input = "let = 42;";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(
            result.is_err(),
            "Parser should fail on missing variable name"
        );
    }

    #[test]
    fn test_invalid_missing_semicolon() {
        let input = "let x = 42";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_err(), "Parser should fail on missing semicolon");
    }

    #[test]
    fn test_invalid_unterminated_string() {
        let input = "let s = \"unterminated;";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(result.is_err(), "Parser should fail on unterminated string");
    }

    #[test]
    fn test_invalid_function_missing_paren() {
        let input = "fn main { let x = 1; }";
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_statement();
        assert!(
            result.is_err(),
            "Parser should fail on missing parentheses in function declaration"
        );
    }
}
