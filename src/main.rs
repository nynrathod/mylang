mod analyzer;
mod lexar;
mod parser;

use analyzer::SemanticAnalyzer;
use lexar::lexer::lex;
use parser::ast::AstNode;
use parser::Parser;
use std::fs;

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

fn process_statement_in_dev_mode(parser: &mut Parser, analyzer: &mut SemanticAnalyzer) -> bool {
    match parser.parse_statement() {
        Ok(mut stmt) => match analyzer.analyze_node(&mut stmt) {
            Ok(_) => {
                println!("PASS");
                false
            }
            Err(e) => {
                println!("FAIL: {:?}", e);
                true
            }
        },
        Err(e) => {
            println!("FAIL: {:?\n}", e);
            skip_to_next_statement(parser);
            true
        }
    }
}

fn main() {
    let input = fs::read_to_string("./test_cases.mylang").unwrap();
    // println!("Source code:\n{}", input);
    let tokens = lex(&input);

    // for token in &tokens {
    //     println!("{:?}", token);
    // }

    // Create parser instance

    const DEV_MODE: bool = true;

    if DEV_MODE {
        // DEV MODE: Parse and analyze statements individually
        let mut parser = Parser::new(&tokens);
        let mut analyzer = SemanticAnalyzer::new();
        let mut error_count = 0;

        while parser.current < parser.tokens.len() {
            if process_statement_in_dev_mode(&mut parser, &mut analyzer) {
                error_count += 1;
            }
        }

        if error_count > 0 {
            println!("DEV_MODE analysis finished with {} errors", error_count);
        } else {
            println!("Semantic analysis passed");
        }
    } else {
        // PRODUCTION MODE: Original behavior
        let mut parser = Parser::new(&tokens);
        let mut ast = match parser.parse_program() {
            Ok(program) => program,
            Err(e) => {
                eprintln!("Parse error: {:?}", e);
                return;
            }
        };
        println!("AST before semantic analysis: {:#?}", ast);

        if let AstNode::Program(ref mut nodes) = ast {
            let mut analyzer = SemanticAnalyzer::new();
            if let Err(e) = analyzer.analyze_program(nodes) {
                eprintln!("Semantic error: {:?}", e);
                return;
            }
            println!("Semantic analysis passed");
        } else {
            eprintln!("Parser did not return a Program node");
        }
    }
}
