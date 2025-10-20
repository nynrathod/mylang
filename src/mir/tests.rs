#[cfg(test)]
mod mir_tests {
    use crate::analyzer::SemanticAnalyzer;
    use crate::lexar::lexer::lex;
    use crate::mir::builder::MirBuilder;
    use crate::parser::Parser;

    fn build_mir(input: &str) -> Result<MirBuilder, String> {
        let tokens = lex(input);
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_program();

        match result {
            Ok(mut ast) => {
                let mut analyzer = SemanticAnalyzer::new(None);
                if let crate::parser::ast::AstNode::Program(ref mut nodes) = ast {
                    analyzer
                        .analyze_program(nodes)
                        .map_err(|e| format!("{:?}", e))?;

                    let mut mir_builder = MirBuilder::new();
                    mir_builder.build_program(nodes);
                    mir_builder.finalize();
                    Ok(mir_builder)
                } else {
                    Err("Not a program".to_string())
                }
            }
            Err(e) => Err(format!("Parse error: {:?}", e)),
        }
    }

    // --- VALID TESTS ---
    #[test]
    fn test_simple_function_mir() {
        let input = r#"
            fn main() {
                let x = 42;
            }
        "#;
        let result = build_mir(input);
        assert!(result.is_ok());
        let mir = result.unwrap();
        assert!(mir.program.functions.iter().any(|f| f.name == "main"));
    }

    #[test]
    fn test_function_with_params_mir() {
        let input = r#"
            fn add(x: Int, y: Int) -> Int {
                return x + y;
            }
            fn main() { }
        "#;
        let result = build_mir(input);
        assert!(result.is_ok());
        let mir = result.unwrap();
        assert!(mir.program.functions.iter().any(|f| f.name == "add"));
        let add_fn = mir
            .program
            .functions
            .iter()
            .find(|f| f.name == "add")
            .unwrap();
        assert_eq!(add_fn.params.len(), 2);
    }

    #[test]
    fn test_variable_assignment_mir() {
        let input = r#"
            fn main() {
                let x = 10;
                let y = x;
            }
        "#;
        let result = build_mir(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_binary_operation_mir() {
        let input = r#"
            fn main() {
                let x = 5 + 3;
                let y = x * 2;
            }
        "#;
        let result = build_mir(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_if_statement_mir() {
        let input = r#"
            fn main() {
                if true {
                    let x = 1;
                } else {
                    let y = 2;
                }
            }
        "#;
        let result = build_mir(input);
        assert!(result.is_ok());
        let mir = result.unwrap();
        let main_fn = mir
            .program
            .functions
            .iter()
            .find(|f| f.name == "main")
            .unwrap();
        // Should have multiple blocks for if/else branches
        assert!(main_fn.blocks.len() > 1);
    }

    #[test]
    fn test_for_loop_mir() {
        let input = r#"
            fn main() {
                for i in 0..10 {
                    print(i);
                }
            }
        "#;
        let result = build_mir(input);
        assert!(result.is_ok());
        let mir = result.unwrap();
        let main_fn = mir
            .program
            .functions
            .iter()
            .find(|f| f.name == "main")
            .unwrap();
        // Loop should create multiple blocks
        assert!(main_fn.blocks.len() > 1);
    }

    #[test]
    fn test_function_call_mir() {
        let input = r#"
            fn getValue() -> Int {
                return 42;
            }
            fn main() {
                let x = getValue();
            }
        "#;
        let result = build_mir(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_array_literal_mir() {
        let input = r#"
            fn main() {
                let arr = [1, 2, 3];
            }
        "#;
        let result = build_mir(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_map_literal_mir() {
        let input = r#"
            fn main() {
                let m = {"key": 42};
            }
        "#;
        let result = build_mir(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_return_statement_mir() {
        let input = r#"
            fn getValue() -> Int {
                return 42;
            }
            fn main() { }
        "#;
        let result = build_mir(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_comparison_mir() {
        let input = r#"
            fn main() {
                let b = 5 > 3;
            }
        "#;
        let result = build_mir(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_nested_blocks_mir() {
        let input = r#"
            fn main() {
                if true {
                    if false {
                        let x = 1;
                    }
                }
            }
        "#;
        let result = build_mir(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_functions_mir() {
        let input = r#"
            fn foo() -> Int { return 1; }
            fn bar() -> Int { return 2; }
            fn main() {
                let x = foo();
                let y = bar();
            }
        "#;
        let result = build_mir(input);
        assert!(result.is_ok());
        let mir = result.unwrap();
        assert!(mir.program.functions.iter().any(|f| f.name == "foo"));
        assert!(mir.program.functions.iter().any(|f| f.name == "bar"));
        assert!(mir.program.functions.iter().any(|f| f.name == "main"));
    }

    // Edge case: nested for loops
    #[test]
    fn test_nested_for_loops_mir() {
        let input = r#"
            fn main() {
                for i in 0..3 {
                    for j in 0..2 {
                        print(i + j);
                    }
                }
            }
        "#;
        let result = build_mir(input);
        assert!(result.is_ok());
    }

    // Edge case: break/continue in nested loops
    #[test]
    fn test_break_continue_nested_loops_mir() {
        let input = r#"
            fn main() {
                for i in 0..5 {
                    for j in 0..5 {
                        if j == 2 { break; }
                        if i == 3 { continue; }
                    }
                }
            }
        "#;
        let result = build_mir(input);
        assert!(result.is_ok());
    }

    // --- INVALID TESTS ---
    #[test]
    fn test_immutable_assignment_mir() {
        let input = "fn main() { let x = 5; x = 10; }";
        let result = build_mir(input);
        assert!(
            result.is_err(),
            "Should fail on assignment to immutable variable"
        );
    }

    #[test]
    fn test_if_condition_not_bool_mir() {
        let input = "fn main() { if 42 { print(1); } }";
        let result = build_mir(input);
        assert!(result.is_err(), "Should fail if condition is not bool");
    }
}
