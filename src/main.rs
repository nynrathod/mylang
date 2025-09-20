mod analyzer;
mod lexar;
mod parser;

use analyzer::SemanticAnalyzer;
use lexar::lexer::lex;
use parser::ast::AstNode;
use parser::Parser;

fn main() {
    // let input = r#"
    //     // Variable declarations and assignments
    //     let a = 5;
    //     var b = "hello";
    //     let isAdmin: Bool = true;
    //     let age: Int = 42;
    //     let nums: Array<Int> = [1, 2, 3, 4];
    //     let user: Map<String, String> = {"name": "Alice", "role": "admin"};
    //     let userName: String = "Nayan";
    //     let userData: Array<String> = ["Nayan", "Rathod"];
    //     let a = b + c;
    //     let a = b - c;
    //     let a = b * c;
    //     let a = b / c;
    //     let a = b % c;

    //     let a = !b;
    //     let a = b < c;
    //     let a = b > c;
    //     let a = b & c;
    //     let a = b | c;
    //     let a = b = c;
    //     let a = b == c;
    //     let a = b === c;
    //     let a = b != c;
    //     let a = b !== c;
    //     let a = b >= c;
    //     let a = b <= c;
    //     let a = b && c;
    //     let a = b || c;
    //     let a = b += c;
    //     let a = b -= c;
    //     let a = b *= c;
    //     let a = b /= c;
    //     let a = b %= c;
    //     let a = b -> c;
    //     let a = b => c;

    //     // If-else statements
    //     if a > b { }
    //     else if b < c {
    //         let nums: Array<Int> = [1, 2, 3, 4];
    //     }
    //     else if c == d { }
    //     else if d != e { }
    //     else if e >= f { }
    //     else if f <= g { }
    //     else if g && h { }
    //     else if i || j { }
    //     else if k + l { }
    //     else if m - n { }
    //     else if o * p { }
    //     else if q / r { }
    //     else if s % t { }
    //     else if !u { }
    //     else if v & w { }
    //     else if x | y { }
    //     else if z = aa { }
    //     else if ab === ac { }
    //     else if ad !== ae { }
    //     else if af += ag { }
    //     else if ah -= ai { }
    //     else if aj *= ak { }
    //     else if al /= am { }
    //     else if an %= ao { }
    //     else if ap -> aq { }
    //     else if ar => as { }
    //     else { }
    //     if a > b { }
    //     else {}
    //     if b > c {
    //         if a > b {
    //             let c = 11-2;
    //         }
    //     }

    //     // Function declarations
    //     fn GetUser(a: Int, b: Int) -> String {}
    //     fn GetUser(a: String) -> String {}
    //     fn GetUser(a: Int, b: Int) -> Int {}
    //     fn GetUser(a: String) -> String {}
    //     fn GetUser(a: Int, b: Int) {}
    //     fn GetUser(a: String) {}
    //     fn GetUser() {}
    //     fn GetUser() -> Int {}
    //     fn GetUser(a: Array<Int>) -> Array<Int> {}
    //     fn GetUser(a: Map<String, Int>) -> Map<String, Int> {}
    //     fn GetUser(a: Int, b: String) -> Bool {}
    //     fn GetUser(a: Bool) -> Bool {}
    //     fn GetUser(a: Int) {}
    //     fn GetUser(a: Int) -> Int {}
    //     fn GetUser(a: String, b: String) -> String {}
    //     fn GetUser(a: Array<String>) -> Array<String> {}

    //     fn GetUser(a: Int, b: Int, c: Int) -> Int {}
    //     fn GetUser(a: Int, b: Int, c: Int) {}
    //     fn getUser(a: Map<String, String>) -> Map<String, String> {
    //         let nums: Array<Int> = [1, 2, 3, 4];
    //         return nums, 5;
    //     }

    //     // For loops
    //     for i in 0..10 {
    //         if b > c {
    //             continue;
    //             if a > b {
    //                 let c = 11-2;
    //                 break;
    //             }
    //         }
    //     }

    //     for i in 0..=10 {

    //     }

    //     let arr = [1,2,3];
    //     for item in arr {

    //     }

    //     let item = {"a": 1, "b": 2};
    //     for key, value in item {
    //     }

    //     for {

    //     }

    //     for (key, value) in item {

    //     }

    //     for Some(x) in maybeValue {

    //     }

    //     // Print statements
    //     print();
    //     print("");
    //     print("Hello");
    //     print(5);

    //     print(a);
    //     print(myVar);
    //     print(a, b, "hello", 5);
    //     print("Value:", x);
    //     print(a + b);
    //     print(f(x, y));
    //     print(true);
    //     print(false);
    //     print(None);

    //     // Multiple assignment and underscore
    //     // sadkjhas
    //     data, info, user = GetUser(5); // newcom
    //     _, info = GetUser();
    //     x = 5;
    // "#;

    // let input = r#"
    //     // Type mismatch: assigning Int to String
    //     // let a: String = 5;

    //     // Type mismatch: assigning Bool to Int
    //     // let b: Int = true;

    //     // Type mismatch: assigning Array<Int> to Map<String, Int>
    //     // let c: Map<String, Int> = [1, 2, 3];

    //     // Type mismatch: assigning String to Array<String>
    //     // let d: Array<String> = "hello";

    //     // Redeclaration: variable 'e' declared twice in same scope
    //     // let e: Int = 10;
    //     // let e: Int = 20;

    //     // Redeclaration: variable 'f' declared twice with different types
    //     // let f: String = "abc";
    //     // let f: Int = 123;

    //     // Undeclared variable usage: 'g' is not declared
    //     // let h: Int = g;

    //     // Undeclared variable usage: 'i' is not declared
    //     // let j: String = i;

    //     // Correct declaration for control
    //     let k: Int = 42;
    //     let l: String = "hello";
    //     let m: Bool = false;
    //     let n: Array<Int> = [1, 2, 3];
    //     let o: Map<String, Int> = {"a": 1, "b": 2};
    // "#;

    // let input = r#"
    //     //  Valid condition type
    //     // let a: Bool = true;
    //     // if a {
    //     //     let x: Int = 10;
    //     // } else {
    //     //     let y: String = "ok";
    //     // }

    //     //  Invalid condition type (Int instead of Bool)
    //     // let b: Int = 5;
    //     // if b {
    //     //     let wrong: Int = 99;
    //     // }

    //     // Undeclared variable in condition
    //     // if undeclared {
    //     //     let c: Int = 1;
    //     // }

    //     //  Undeclared variable in then block
    //     // let d: Bool = true;
    //     // if d {
    //     //     let e: Int = unknownvar;
    //     // }

    //     //  Undeclared variable in else block
    //     // let f: Bool = true;
    //     // if f {
    //     //     let g: Int = 1;
    //     // } else {
    //     //     let h: String = missing;
    //     // }

    //     //  Type mismatch inside blocks
    //     // let i: Bool = true;
    //     // if i {
    //     //     let j: Int = "oops";     // mismatch
    //     // } else {
    //     //     let k: String = 42;      // mismatch
    //     // }

    //     //  Nested if inside then
    //     // let l: Bool = true;
    //     // let m: Bool = false;
    //     // if l {
    //     //     if m {
    //     //         let n: Int = 123;
    //     //     }
    //     // }

    //     //  Nested if with invalid condition
    //     // let o: Bool = true;
    //     // let p: Int = 42;
    //     // if o {
    //     //     if p {
    //     //         let q: Int = 99;
    //     //     }
    //     // }

    //     //  If without else
    //     // let r: Bool = true;
    //     // if r {
    //     //     let msg: String = "hello";
    //     // }

    //     // If-else chain
    //     // let s: Bool = true;
    //     // let t: Bool = false;
    //     // if s {
    //     //     let u: Int = 10;
    //     // } else if t {
    //     //     let v: Int = 20;
    //     // } else {
    //     //     let w: Int = 30;
    //     // }
    // "#;

    // let input = r#"

    //     // let a = 5;
    //     // let a = 5;
    //     // if a == 5 {

    //     // }
    //     // fn Foo() {}

    //     // fn Foo1(a: Int) {}
    //     // fn Foo2(a: Int, b: String) -> (Int, String) { return a, "sfsd"; }

    //     // fn Bar1(a: String) -> String { return a; }
    //     // fn Bar2(a: Int, b: Int) -> Int { return a + b; }
    //     // fn Baz1(a: Array<Int>) -> Array<Int> { return a; }
    //     // fn Baz2(a: Map<String, Int>) -> Map<String, Int> { return a; }

    //     // fn Cond() -> Int {
    //     //     if true { return 1; }
    //     //     else { return 2; }
    //     // }

    //     // fn ScopeTest() {
    //     //     let y: Int = 5;
    //     //     print(y);
    //     // }
    //     // fn Shadow(a: Int) {
    //     //     let b: Int = 99;
    //     // }

    //     // fn Overload1(a: Int) -> Int { return a; }
    //     // fn Overload2(a: String) -> String { return a; }

    //     // fn Rec(x: Int) -> Int { return Rec(x - 1); }

    //     //  Return type mismatch
    //     // fn foo() {
    //     //     return 42;
    //     // }
    //     // fn WrongReturn2() -> Int { return "hi"; }
    //     // fn WrongReturn3() { return 5; }
    //     // fn WrongReturn4() -> Int {
    //     //     if true { return 1; }
    //     //     else { return "oops"; }
    //     // }

    //     //  Function redeclaration conflict
    //     // fn Dup(a: Int) -> Int { return a; }
    //     // fn Dup(a: Int) -> String { return a; }

    //     // Duplicate parameter names
    //     // fn DupParam(a: Int, a: Int) {}

    //     // To implement later
    //     // let outer: Int = 10;
    //     // fn ShadowOuter() -> Int { return outer; } // outer not in function scope

    //     // fn UseUndeclared() {
    //     //     print(z);  // z not declared
    //     // }
    //     "#;

    let input = r#"
        let b = "s";
        let mut a = "s";
        // Valid cases
        // fn GetValue() -> Int{ return 5; }
        // a = GetValue();

        // Multiple variables from function returning tuple
        // a, b = GetUser(5);

        // Multiple variables with wildcards
        // a, _, c = GetUser(5);

        // fn SomeFunction2(a: Int, b: Int) -> (Int, String) { return 5, "s"; }
        // a, b = SomeFunction2(1, 2);

        // fn SomeFunction(a: Int, b: Int) -> (Int, String) { return 5, "s"; }
        // let mut a, _ = SomeFunction(1, 2);

        // fn SomeFunction8(a: Int, b: Int) -> (Int, String, String) { return 5, "s", "sad"; }
        // let  (a, (b, _)) = SomeFunction8(1, 2);



        // invalid

        // wrong function
        // a, b = 42;
        // a, b = "sdas";

         // function not declared
        // a, b = UnknownFunc(5);



        // fn SomeFunction3(a: Int, b: Int) -> (Int, String) { return 5, "s"; }
        // a = SomeFunction3(1, 2);

        // fn SomeFunction4(a: Int, b: Int) -> (Int, String) { return 5, "s"; }
        // a, b = SomeFunction4(1);

        // fn SomeFunction6(a: Int, b: Int) -> (Int, String) { return 5, "s"; }
        // a, b = SomeFunction6(1, "Sda");

        // fn SomeFunction5(a: Int, b: Int) -> Int { return 5; }
        // let d, c = SomeFunction5(1, 2);


        // if, else = GetUser(5);
        "#;

    // let input = fs::read_to_string("./syntax.mylang").unwrap();
    // println!("Source code:\n{}", input);
    let tokens = lex(&input);

    // for token in &tokens {
    //     println!("{:?}", token);
    // }

    // Create parser instance

    // Parse the whole program
    let mut parser = Parser::new(&tokens);
    let mut ast = match parser.parse_program() {
        Ok(program) => program,
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
            return;
        }
    };
    println!("AST before semantic analysis: {:#?}", ast);

    let mut analyzer = SemanticAnalyzer::new();
    if let AstNode::Program(ref mut nodes) = ast {
        match analyzer.analyze_program(nodes) {
            Ok(_) => {
                println!("Semantic analysis passed!");
                println!("AST after analysis (types inferred/appended): {:#?}", ast);
            }

            Err(e) => eprintln!("Semantic error: {:?}", e),
        }
    } else {
        eprintln!("Parser did not return a Program node");
    }
}
