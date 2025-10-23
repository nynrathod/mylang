#[cfg(test)]
mod analyzer_tests {
    use crate::analyzer::SemanticAnalyzer;
    use crate::lexar::lexer::lex;
    use crate::parser::Parser;

    fn analyze_code(input: &str) -> Result<(), String> {
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_program();

        match result {
            Ok(mut ast) => {
                let mut analyzer = SemanticAnalyzer::new(None);
                if let crate::parser::ast::AstNode::Program(ref mut nodes) = ast {
                    analyzer
                        .analyze_program(nodes)
                        .map_err(|e| format!("{:?}", e))
                } else {
                    Err("Not a program".to_string())
                }
            }
            Err(e) => Err(format!("Parse error: {:?}", e)),
        }
    }

    // --- VALID TESTS ---
    #[test]
    fn test_valid_variable_declaration() {
        let input = "fn main() { let x: Int = 42; }";
        assert!(analyze_code(input).is_ok());
    }

    #[test]
    fn test_valid_function_call() {
        let input = r#"
            fn getValue() -> Int { return 42; }
            fn main() { let x = getValue(); }
        "#;
        assert!(analyze_code(input).is_ok());
    }

    #[test]
    fn test_mutable_assignment() {
        let input = "fn main() { let mut x = 5; x = 10; }";
        assert!(analyze_code(input).is_ok());
    }

    #[test]
    fn test_valid_if_condition() {
        let input = "fn main() { if true { print(1); } }";
        assert!(analyze_code(input).is_ok());
    }

    #[test]
    fn test_valid_comparison() {
        let input = "fn main() { let b = 5 > 3; }";
        assert!(analyze_code(input).is_ok());
    }

    #[test]
    fn test_array_type_checking() {
        let input = "fn main() { let arr: [Int] = [1, 2, 3]; }";
        assert!(analyze_code(input).is_ok());
    }

    #[test]
    fn test_map_type_checking() {
        let input = r#"fn main() { let m: {Str: Int} = {"a": 1, "b": 2}; }"#;
        assert!(analyze_code(input).is_ok());
    }

    #[test]
    fn test_for_range_loop() {
        let input = "fn main() { for i in 0..10 { print(i); } }";
        assert!(analyze_code(input).is_ok());
    }

    #[test]
    fn test_for_array_loop() {
        let input = "fn main() { let arr = [1, 2, 3]; for item in arr { print(item); } }";
        assert!(analyze_code(input).is_ok());
    }

    #[test]
    fn test_for_map_loop() {
        let input = r#"fn main() { let m = {"a": 1}; for (k, v) in m { print(k); } }"#;
        assert!(analyze_code(input).is_ok());
    }

    #[test]
    fn test_nested_scopes() {
        let input = r#"
            fn main() {
                let x = 5;
                if true {
                    let y = x;
                    print(y);
                }
            }
        "#;
        assert!(analyze_code(input).is_ok());
    }

    #[test]
    fn test_binary_operations() {
        let input = "fn main() { let x = 5 + 3 * 2 - 1; }";
        assert!(analyze_code(input).is_ok());
    }

    #[test]
    fn test_string_concatenation() {
        let input = r#"fn main() { let s = "hello" + " world"; }"#;
        assert!(analyze_code(input).is_ok());
    }

    #[test]
    fn test_valid_break_in_loop() {
        let input = "fn main() { for i in 0..10 { if i == 5 { break; } } }";
        assert!(analyze_code(input).is_ok());
    }

    #[test]
    fn test_valid_continue_in_loop() {
        let input = "fn main() { for i in 0..10 { if i == 5 { continue; } print(i); } }";
        assert!(analyze_code(input).is_ok());
    }

    // --- EDGE CASES ---
    #[test]
    fn test_variable_shadowing_in_inner_scope() {
        let input = r#"
            fn main() {
                let x = 1;
                if true {
                    let x = 2;
                    print(x);
                }
                print(x);
            }
        "#;
        assert!(analyze_code(input).is_ok());
    }

    // --- INVALID TESTS ---
    #[test]
    fn test_type_mismatch() {
        let input = "fn main() { let x: Int = \"hello\"; }";
        assert!(analyze_code(input).is_err());
    }

    #[test]
    fn test_undeclared_variable() {
        let input = "fn main() { let x = y; }";
        assert!(analyze_code(input).is_err());
    }

    #[test]
    fn test_function_wrong_arg_count() {
        let input = r#"
            fn add(x: Int, y: Int) -> Int { return x + y; }
            fn main() { let x = add(5); }
        "#;
        assert!(analyze_code(input).is_err());
    }

    #[test]
    fn test_function_wrong_arg_type() {
        let input = r#"
            fn add(x: Int, y: Int) -> Int { return x + y; }
            fn main() { let x = add(5, "hello"); }
        "#;
        assert!(analyze_code(input).is_err());
    }

    #[test]
    fn test_return_type_mismatch() {
        let input = r#"
            fn getValue() -> Int { return "hello"; }
            fn main() { }
        "#;
        assert!(analyze_code(input).is_err());
    }

    #[test]
    fn test_immutable_assignment_error() {
        let input = "fn main() { let x = 5; x = 10; }";
        assert!(analyze_code(input).is_err());
    }

    #[test]
    fn test_if_condition_must_be_bool() {
        let input = "fn main() { if 42 { print(1); } }";
        assert!(analyze_code(input).is_err());
    }

    #[test]
    fn test_array_type_mismatch() {
        let input = r#"fn main() { let arr: [Int] = ["a", "b"]; }"#;
        assert!(analyze_code(input).is_err());
    }

    #[test]
    fn test_duplicate_function_error() {
        let input = r#"
            fn test() -> Int { return 1; }
            fn test() -> Int { return 2; }
            fn main() { }
        "#;
        assert!(analyze_code(input).is_err());
    }

    #[test]
    fn test_scope_isolation() {
        let input = r#"
            fn main() {
                if true {
                    let x = 5;
                }
                let y = x;
            }
        "#;
        assert!(analyze_code(input).is_err());
    }

    #[test]
    fn test_invalid_duplicate_variable() {
        let input = "fn main() { let x = 1; let x = 2; }";
        assert!(
            analyze_code(input).is_err(),
            "Should fail on duplicate variable declaration"
        );
    }

    #[test]
    fn test_invalid_duplicate_parameter() {
        let input = "fn foo(x: Int, x: Int) { } fn main() { }";
        assert!(
            analyze_code(input).is_err(),
            "Should fail on duplicate function parameter"
        );
    }

    #[test]
    fn test_invalid_break_outside_loop() {
        let input = "fn main() { break; }";
        assert!(
            analyze_code(input).is_err(),
            "Should fail on break outside loop"
        );
    }

    #[test]
    fn test_invalid_continue_outside_loop() {
        let input = "fn main() { continue; }";
        assert!(
            analyze_code(input).is_err(),
            "Should fail on continue outside loop"
        );
    }

    #[test]
    fn test_invalid_function_return_missing() {
        let input = r#"
            fn foo() -> Int { }
            fn main() { }
        "#;
        assert!(
            analyze_code(input).is_err(),
            "Should fail if function with return type does not return"
        );
    }

    // --- EDGE INVALID CASES ---
    #[test]
    fn test_import_missing_module() {
        let input = r#"import missing::Module;"#;
        assert!(analyze_code(input).is_err());
    }

    #[test]
    fn test_variable_shadowing_error() {
        let input = r#"
            fn main() {
                let x = 1;
                let x = 2;
            }
        "#;
        assert!(analyze_code(input).is_err());
    }
}
