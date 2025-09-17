mod lexar;
mod parser;

use lexar::lexer::lex;
use parser::Parser;

fn main() {
    let input = "enum UserProfile {
        name,
        age(Map<String,Int>)
    }";

    // let input = fs::read_to_string("./syntax.mylang").unwrap();
    // println!("Source code:\n{}", input);
    let tokens = lex(&input);

    for token in &tokens {
        println!("{:?}", token);
    }

    // Create parser instance

    let mut parser = Parser::new(&tokens);

    // Parse the whole program
    match parser.parse_program() {
        Ok(program) => println!("{:#?}", program),
        Err(e) => eprintln!("Parse error: {:?}", e),
    }
}
