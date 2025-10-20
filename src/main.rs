use mylang::analyzer::SemanticAnalyzer;
use mylang::codegen::core::CodeGen;
use mylang::diagnostics::{
    print_grouped, print_note, print_parse_error_with_source, print_semantic_error,
    DiagnosticRecord,
};
use mylang::lexar::lexer::lex;
use mylang::lexar::token::{Token, TokenType};
use mylang::mir::builder::MirBuilder;
use mylang::parser::ast::AstNode;
use mylang::parser::ParseError;
use mylang::parser::Parser;
use std::collections::HashMap;
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
    diagnostics: &mut Vec<DiagnosticRecord>,
    input_path: &str,
) -> bool {
    let start_index = parser.current;

    match parser.parse_statement() {
        Ok(mut stmt) => {
            let end_index = parser.current;
            let code_line = tokens_to_source_line(&parser.tokens[start_index..end_index]);

            // println!("CODE: {}", code_line);

            let failed = match analyzer.analyze_node(&mut stmt) {
                Ok(_) => {
                    // println!("PASS\n");
                    false
                }
                Err(e) => {
                    diagnostics.push(DiagnosticRecord {
                        filename: input_path.to_string(),
                        message: e.to_string(),
                        line: None,
                        col: None,
                        is_parse: false,
                    });
                    // println!("");
                    true
                }
            };

            statements.push(stmt);
            failed
        }
        Err(e) => {
            // Capture parse error with position if present
            let (line, col, msg) = match &e {
                ParseError::UnexpectedTokenAt { msg, line, col } => {
                    (Some(*line), Some(*col), msg.clone())
                }
                _ => (None, None, e.to_string()),
            };
            diagnostics.push(DiagnosticRecord {
                filename: input_path.to_string(),
                message: msg,
                line,
                col,
                is_parse: true,
            });
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

    // Use the myproject directory as the project root for module resolution
    let project_root = PathBuf::from(input_path).parent().unwrap().to_path_buf();

    let tokens = lex(&input);
    let mut parser = Parser::new(&tokens);
    let mut analyzer = SemanticAnalyzer::new(Some(project_root));

    let mut error_count = 0;
    let mut statements = Vec::new();

    if DEV_MODE {
        let mut diagnostics: Vec<DiagnosticRecord> = Vec::new();
        // DEV mode: parse each statement, do not analyze yet
        while parser.current < parser.tokens.len() {
            match parser.parse_statement() {
                Ok(stmt) => statements.push(stmt),
                Err(e) => {
                    let (line, col, msg) = match &e {
                        ParseError::UnexpectedTokenAt { msg, line, col } => {
                            (Some(*line), Some(*col), msg.clone())
                        }
                        _ => (None, None, e.to_string()),
                    };
                    diagnostics.push(DiagnosticRecord {
                        filename: input_path.to_string(),
                        message: msg,
                        line,
                        col,
                        is_parse: true,
                    });
                    skip_to_next_statement(&mut parser);
                    error_count += 1;
                }
            }
        }

        // Now analyze the whole program (imports, functions, calls)
        let mut program_ast_nodes = statements.clone();
        if let Err(e) = analyzer.analyze_program(&mut program_ast_nodes) {
            diagnostics.push(DiagnosticRecord {
                filename: input_path.to_string(),
                message: e.to_string(),
                line: None,
                col: None,
                is_parse: false,
            });
            error_count += 1;
        }

        if PRINT_AST {
            let program_ast = AstNode::Program(program_ast_nodes.clone());
            println!("\nComplete Program AST in DEV_MODE:");
            println!("{:#?}", program_ast);
        }

        println!("\nDEV_MODE analysis finished with {} errors", error_count);

        // Grouped diagnostic output by file with carets
        let mut sources = HashMap::new();
        sources.insert(input_path.to_string(), input.clone());
        print_grouped(&diagnostics, &sources);

        // Halt pipeline if errors were found, skipping MIR/IR generation
        if error_count > 0 {
            println!(
                "\nAborting: {} errors found, skipping MIR/IR generation.",
                error_count
            );
            return;
        }

        // Optionally run MIR/codegen in dev mode for debugging
        // Include imported functions in MIR/codegen input, just like in production mode
        let mut all_nodes = analyzer.imported_functions.clone();
        all_nodes.extend(program_ast_nodes.clone());

        let mut mir_builder = MirBuilder::new();
        mir_builder.build_program(&all_nodes);
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
                            let mut sources = HashMap::new();
                            sources.insert(input_path.to_string(), input.clone());
                            let diagnostics = vec![DiagnosticRecord {
                                filename: input_path.to_string(),
                                message: e.to_string(),
                                line: None,
                                col: None,
                                is_parse: false,
                            }];
                            print_grouped(&diagnostics, &sources);
                            return;
                        }
                    }
                }
            }
            Err(e) => {
                let mut sources = HashMap::new();
                sources.insert(input_path.to_string(), input.clone());
                let (line, col, msg) = match &e {
                    ParseError::UnexpectedTokenAt { msg, line, col } => {
                        (Some(*line), Some(*col), msg.clone())
                    }
                    _ => (None, None, e.to_string()),
                };
                let diagnostics = vec![DiagnosticRecord {
                    filename: input_path.to_string(),
                    message: msg,
                    line,
                    col,
                    is_parse: true,
                }];
                print_grouped(&diagnostics, &sources);
            }
        }
    }
}
