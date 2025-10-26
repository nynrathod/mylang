use doo::analyzer::SemanticAnalyzer;
use doo::codegen::core::CodeGen;
use doo::lexar::lexer::lex;
use doo::mir::builder::MirBuilder;
use doo::parser::Parser;
use inkwell::context::Context;

fn compile_full_pipeline(input: &str) -> Result<String, String> {
    let tokens = lex(input);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse_program();

    match result {
        Ok(mut ast) => {
            let mut analyzer = SemanticAnalyzer::new(None);
            if let doo::parser::ast::AstNode::Program(ref mut nodes) = ast {
                analyzer
                    .analyze_program(nodes)
                    .map_err(|e| format!("{:?}", e))?;

                let mut mir_builder = MirBuilder::new();
                mir_builder.build_program(nodes);
                mir_builder.finalize();

                let context = Context::create();
                let mut codegen = CodeGen::new("integration_test", &context);
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
fn test_complete_program() {
    let input = r#"
        fn add(x: Int, y: Int) -> Int {
            return x + y;
        }
        fn main() {
            let result = add(5, 3);
            print(result);
        }
    "#;
    assert!(compile_full_pipeline(input).is_ok());
}

#[test]
fn test_loops_and_conditionals() {
    let input = r#"
        fn main() {
            for i in 0..10 {
                if i > 5 {
                    print(i);
                }
            }
        }
    "#;
    assert!(compile_full_pipeline(input).is_ok());
}

#[test]
fn test_nested_functions() {
    let input = r#"
        fn helper() -> Int { return 10; }
        fn compute() -> Int {
            let x = helper();
            return x * 2;
        }
        fn main() {
            let result = compute();
            print(result);
        }
    "#;
    assert!(compile_full_pipeline(input).is_ok());
}
