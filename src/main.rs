mod lexer;
mod tokens;

use std::{collections::HashMap, fs};
use tokens::{Token, TokenType};

fn main() {
    // let input = "user : Map<String, String> = {\"name\": \"Alice\", \"role\": \"admin\"} ";
    // let input = fs::read_to_string("./syntax.mylang").unwrap();
    // println!("Source code:\n{}", input);
    let tokens = lexer::lex("let a = 5;");

    for token in &tokens {
        println!("{:?}", token);
    }
}
