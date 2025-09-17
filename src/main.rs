mod lexer;
mod parser;
mod tokens;

use parser::Parser;

fn main() {
    let input = r#"let user: Map<String, Int> = {
        "age": 42,
        "score": 100,
        "level": 5
    };"#;

    // let input = fs::read_to_string("./syntax.mylang").unwrap();
    // println!("Source code:\n{}", input);
    let tokens = lexer::lex(&input);
    let mut parser = Parser::new(&tokens);

    match parser.parse_program() {
        Ok(program) => println!("{:#?}", program),
        Err(e) => eprintln!("Parse error: {:?}", e),
    }

    // for token in &tokens {
    //     println!("{:?}", token);
    // }
}
