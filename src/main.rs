use mylang::analyzer::SemanticAnalyzer;
use mylang::codegen::core::CodeGen;
use mylang::lexar::lexer::lex;
use mylang::lexar::token::{Token, TokenType};
use mylang::mir::builder::MirBuilder;
use mylang::parser::ast::AstNode;
use mylang::parser::Parser;
use std::fs;
use std::path::PathBuf;

fn skip_to_next_statement(parser: &mut Parser) {
    while parser.current < parser.tokens.len() {
        if let Some(tok) = parser.peek() {
            if matches!(tok.kind, TokenType::Semi) {
                parser.advance();
                break;
            }
        }
        parser.advance();
    }
}

fn tokens_to_source_line(tokens: &[Token]) -> String {
    tokens
        .iter()
        .map(|tok| match tok.kind {
            TokenType::String => format!("\"{}\"", tok.value),
            _ => tok.value.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn process_statement_in_dev_mode(
    parser: &mut Parser,
    analyzer: &mut SemanticAnalyzer,
    statements: &mut Vec<AstNode>,
) -> bool {
    let start_index = parser.current;

    match parser.parse_statement() {
        Ok(mut stmt) => {
            let end_index = parser.current;
            let code_line = tokens_to_source_line(&parser.tokens[start_index..end_index]);

            println!("CODE: {}", code_line);

            let failed = match analyzer.analyze_node(&mut stmt) {
                Ok(_) => {
                    println!("PASS\n");
                    false
                }
                Err(e) => {
                    println!("FAIL: {:?}\n", e);
                    true
                }
            };

            statements.push(stmt);
            failed
        }
        Err(e) => {
            println!("FAIL: {:?}\n", e);
            skip_to_next_statement(parser);
            true
        }
    }
}

fn main() {
    // Use Rust's built-in debug/release detection for mode
    const DEV_MODE: bool = cfg!(debug_assertions);
    const PRINT_AST: bool = DEV_MODE; // Only print AST in dev mode

    // let input_path = "./examples/myproject/main.my";
    let input_path = "./test_cases.md";
    let input = fs::read_to_string(input_path).unwrap();

    let project_root = PathBuf::from(input_path).parent().unwrap().to_path_buf();

    let tokens = lex(&input);
    let mut parser = Parser::new(&tokens);
    let mut analyzer = SemanticAnalyzer::new(Some(project_root));

    let mut error_count = 0;
    let mut statements = Vec::new();

    if DEV_MODE {
        // DEV mode: process each statement individually, print everything
        while parser.current < parser.tokens.len() {
            if process_statement_in_dev_mode(&mut parser, &mut analyzer, &mut statements) {
                error_count += 1;
            }
        }

        if PRINT_AST {
            let program_ast = AstNode::Program(statements.clone());
            println!("\nComplete Program AST in DEV_MODE:");
            println!("{:#?}", program_ast);
        }

        println!("\nDEV_MODE analysis finished with {} errors", error_count);

        // Optionally run MIR/codegen in dev mode for debugging
        let mut mir_builder = MirBuilder::new();
        mir_builder.build_program(&statements);
        mir_builder.finalize();
        println!("\nGenerated SSA MIR:\n{:#?}", mir_builder.program);

        let context = inkwell::context::Context::create();
        let mut codegen = CodeGen::new("main_module", &context);
        codegen.generate_program(&mir_builder.program);
        println!("Generated LLVM IR:");
        codegen.dump();
    } else {
        // PRODUCTION mode: parse full program at once, minimal output
        match parser.parse_program() {
            Ok(mut program_ast) => {
                if let AstNode::Program(ref mut nodes) = program_ast {
                    match analyzer.analyze_program(nodes) {
                        Ok(_) => {
                            println!("Semantic analysis passed");

                            let mut all_nodes = analyzer.imported_functions.clone();
                            all_nodes.extend(nodes.clone());

                            // Only print AST if explicitly enabled
                            if PRINT_AST {
                                println!(
                                    "AST after semantic analysis (with imports):\n{:#?}",
                                    AstNode::Program(all_nodes.clone())
                                );
                            }

                            // MIR and Codegen always run in prod, but only print IR in dev
                            let mut mir_builder = MirBuilder::new();
                            mir_builder.build_program(&all_nodes);
                            mir_builder.finalize();

                            if DEV_MODE {
                                println!("\nGenerated SSA MIR:\n{:#?}", mir_builder.program);
                            }

                            let context = inkwell::context::Context::create();
                            let mut codegen = CodeGen::new("main_module", &context);
                            codegen.generate_program(&mir_builder.program);

                            if DEV_MODE {
                                println!("Generated LLVM IR:");
                                codegen.dump();
                            }

                            // Always save LLVM IR to file in prod
                            let llvm_ir = codegen.module.print_to_string();
                            std::fs::write("output.ll", llvm_ir.to_string()).unwrap();
                        }
                        Err(e) => {
                            eprintln!("Semantic analysis failed: {:?}", e);
                            return;
                        }
                    }
                }
            }
            Err(e) => eprintln!("Parse error: {:?}", e),
        }
    }
}
