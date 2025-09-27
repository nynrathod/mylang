mod analyzer;
mod codegen;
mod lexar;
mod mir;
mod parser;

use analyzer::SemanticAnalyzer;
use lexar::lexer::lex;
use parser::ast::AstNode;
use parser::Parser;
use std::fs;

use crate::lexar::token::{Token, TokenType};
use codegen::CodeGen;
use mir::builder::MirBuilder;

fn skip_to_next_statement(parser: &mut Parser) {
    while parser.current < parser.tokens.len() {
        if let Some(tok) = parser.peek() {
            if matches!(tok.kind, crate::lexar::token::TokenType::Semi) {
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
    let input = fs::read_to_string("./test_cases.mylang").unwrap();
    let tokens = lex(&input);

    const DEV_MODE: bool = false;
    const PRINT_AST: bool = true;

    let mut parser = Parser::new(&tokens);
    let mut analyzer = SemanticAnalyzer::new();
    let mut error_count = 0;
    let mut statements = Vec::new();

    if DEV_MODE {
        // DEV mode: process each statement individually
        while parser.current < parser.tokens.len() {
            if process_statement_in_dev_mode(&mut parser, &mut analyzer, &mut statements) {
                error_count += 1;
            }
        }

        // Print full AST at the end
        if PRINT_AST {
            let program_ast = AstNode::Program(statements.clone());
            println!("\nComplete Program AST in DEV_MODE:");
            // println!("{:#?}", program_ast);
        }

        println!("\nDEV_MODE analysis finished with {} errors", error_count);
    } else {
        // PRODUCTION mode: parse full program at once, stop on first failure
        match parser.parse_program() {
            Ok(mut program_ast) => {
                if PRINT_AST {
                    println!("AST:\n{:#?}", program_ast);
                }

                if let AstNode::Program(ref mut nodes) = program_ast {
                    match analyzer.analyze_program(nodes) {
                        Ok(_) => {
                            println!("\nSemantic analysis passed");

                            // ===== INTEGRATE MIR =====
                            let mut mir_builder = MirBuilder::new();
                            mir_builder.build_program(nodes);
                            mir_builder.finalize();
                            println!("\nGenerated SSA MIR:\n{:#?}", mir_builder.program);
                            // println!("\nGenerated SSA MIR:\n{}", mir_builder.program);

                            // ===== INTEGRATE CODEGEN =====
                            let context = inkwell::context::Context::create();
                            let mut codegen = CodeGen::new("main_module", &context);
                            codegen.generate_program(&mir_builder.program);

                            println!("Generated LLVM IR:");
                            codegen.dump(); // This prints to stderr

                            // Also save to file
                            let llvm_ir = codegen.module.print_to_string();
                            std::fs::write("output.ll", llvm_ir.to_string()).unwrap();
                            println!("LLVM IR written to output.ll");
                            // =============================
                        }
                        Err(e) => {
                            println!("\nSemantic analysis failed: {:?}", e);
                            return;
                        }
                    }
                }
            }
            Err(e) => eprintln!("Parse error: {:?}", e),
        }
    }
}
