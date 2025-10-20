#[cfg(test)]
mod codegen_tests {
    use crate::analyzer::SemanticAnalyzer;
    use crate::codegen::core::CodeGen;
    use crate::lexar::lexer::lex;
    use crate::mir::builder::MirBuilder;
    use crate::parser::Parser;
    use inkwell::context::Context;

    fn compile_code(input: &str) -> Result<String, String> {
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

                    let context = Context::create();
                    let mut codegen = CodeGen::new("test_module", &context);
                    codegen.generate_program(&mir_builder.program);

                    Ok(codegen.module.print_to_string().to_string())
                } else {
                    Err("Not a program".to_string())
                }
            }
            Err(e) => Err(format!("Parse error: {:?}", e)),
        }
    }

    #[test]
    fn test_simple_function_codegen() {
        let input = r#"fn main() { let x = 42; }"#;
        let result = compile_code(input);
        assert!(result.is_ok());
        let ir = result.unwrap();
        assert!(ir.contains("define"));
        assert!(ir.contains("main"));
    }

    #[test]
    fn test_function_with_return() {
        let input = r#"fn getValue() -> Int { return 42; } fn main() { }"#;
        let result = compile_code(input);
        assert!(result.is_ok());
        let ir = result.unwrap();
        assert!(ir.contains("getValue"));
    }

    #[test]
    fn test_arithmetic_operations() {
        let input = r#"fn main() { let x = 5 + 3; let y = x * 2; }"#;
        let result = compile_code(input);
        assert!(result.is_ok());
        #[test]
        fn test_invalid_syntax_codegen() {
            // Missing variable name
            let input = "fn main() { let = 42; }";
            let result = compile_code(input);
            assert!(result.is_err(), "Should fail on invalid syntax");
        }

        #[test]
        fn test_type_error_codegen() {
            // Assign string to Int variable
            let input = "fn main() { let x: Int = \"hello\"; }";
            let result = compile_code(input);
            assert!(result.is_err(), "Should fail on type error");
        }

        #[test]
        fn test_undefined_variable_codegen() {
            // Use of undeclared variable
            let input = "fn main() { let x = y; }";
            let result = compile_code(input);
            assert!(result.is_err(), "Should fail on undefined variable");
        }

        #[test]
        fn test_wrong_function_arg_count_codegen() {
            let input = r#"
            fn add(x: Int, y: Int) -> Int { return x + y; }
            fn main() { let x = add(5); }
        "#;
            let result = compile_code(input);
            assert!(result.is_err(), "Should fail on wrong argument count");
        }

        #[test]
        fn test_wrong_function_arg_type_codegen() {
            let input = r#"
            fn add(x: Int, y: Int) -> Int { return x + y; }
            fn main() { let x = add(5, "hello"); }
        "#;
            let result = compile_code(input);
            assert!(result.is_err(), "Should fail on wrong argument type");
        }

        #[test]
        fn test_return_type_mismatch_codegen() {
            let input = r#"
            fn getValue() -> Int { return "hello"; }
            fn main() { }
        "#;
            let result = compile_code(input);
            assert!(result.is_err(), "Should fail on return type mismatch");
        }

        #[test]
        fn test_immutable_assignment_codegen() {
            let input = "fn main() { let x = 5; x = 10; }";
            let result = compile_code(input);
            assert!(
                result.is_err(),
                "Should fail on assignment to immutable variable"
            );
        }

        #[test]
        fn test_if_condition_not_bool_codegen() {
            let input = "fn main() { if 42 { print(1); } }";
            let result = compile_code(input);
            assert!(result.is_err(), "Should fail if condition is not bool");
        }

        #[test]
        fn test_if_statement_codegen() {
            let input = r#"fn main() { if true { let x = 1; } }"#;
            let result = compile_code(input);
            assert!(result.is_ok());
            let ir = result.unwrap();
            assert!(ir.contains("br"));
        }

        #[test]
        fn test_for_loop_codegen() {
            let input = r#"fn main() { for i in 0..5 { print(i); } }"#;
            let result = compile_code(input);
            assert!(result.is_ok());
        }

        #[test]
        fn test_function_call_codegen() {
            let input = r#"fn getValue() -> Int { return 42; } fn main() { let x = getValue(); }"#;
            let result = compile_code(input);
            assert!(result.is_ok());
            let ir = result.unwrap();
            assert!(ir.contains("call"));
        }

        #[test]
        fn test_comparison_codegen() {
            let input = r#"fn main() { let b = 5 > 3; }"#;
            let result = compile_code(input);
            assert!(result.is_ok());
            let ir = result.unwrap();
            assert!(ir.contains("icmp"));
        }
    }
}
