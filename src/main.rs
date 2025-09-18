mod lexar;
mod parser;

use lexar::lexer::lex;
use parser::Parser;

fn main() {
    let input = r#"
        // Variable declarations and assignments
        let a = 5;
        var b = "hello";
        let isAdmin: Bool = true;
        let age: Int = 42;
        let nums: Array<Int> = [1, 2, 3, 4];
        let user: Map<String, String> = {"name": "Alice", "role": "admin"};
        let userName: String = "Nayan";
        let userData: Array<String> = ["Nayan", "Rathod"];
        let a = b + c;
        let a = b - c;
        let a = b * c;
        let a = b / c;
        let a = b % c;

        let a = !b;
        let a = b < c;
        let a = b > c;
        let a = b & c;
        let a = b | c;
        let a = b = c;
        let a = b == c;
        let a = b === c;
        let a = b != c;
        let a = b !== c;
        let a = b >= c;
        let a = b <= c;
        let a = b && c;
        let a = b || c;
        let a = b += c;
        let a = b -= c;
        let a = b *= c;
        let a = b /= c;
        let a = b %= c;
        let a = b -> c;
        let a = b => c;

        // If-else statements
        if a > b { }
        else if b < c {
            let nums: Array<Int> = [1, 2, 3, 4];
        }
        else if c == d { }
        else if d != e { }
        else if e >= f { }
        else if f <= g { }
        else if g && h { }
        else if i || j { }
        else if k + l { }
        else if m - n { }
        else if o * p { }
        else if q / r { }
        else if s % t { }
        else if !u { }
        else if v & w { }
        else if x | y { }
        else if z = aa { }
        else if ab === ac { }
        else if ad !== ae { }
        else if af += ag { }
        else if ah -= ai { }
        else if aj *= ak { }
        else if al /= am { }
        else if an %= ao { }
        else if ap -> aq { }
        else if ar => as { }
        else { }
        if a > b { }
        else {}
        if b > c {
            if a > b {
                let c = 11-2;
            }
        }

        // Function declarations
        fn GetUser(a: Int, b: Int) -> String {}
        fn GetUser(a: String) -> String {}
        fn GetUser(a: Int, b: Int) -> Int {}
        fn GetUser(a: String) -> String {}
        fn GetUser(a: Int, b: Int) {}
        fn GetUser(a: String) {}
        fn GetUser() {}
        fn GetUser() -> Int {}
        fn GetUser(a: Array<Int>) -> Array<Int> {}
        fn GetUser(a: Map<String, Int>) -> Map<String, Int> {}
        fn GetUser(a: Int, b: String) -> Bool {}
        fn GetUser(a: Bool) -> Bool {}
        fn GetUser(a: Int) {}
        fn GetUser(a: Int) -> Int {}
        fn GetUser(a: String, b: String) -> String {}
        fn GetUser(a: Array<String>) -> Array<String> {}

        fn GetUser(a: Int, b: Int, c: Int) -> Int {}
        fn GetUser(a: Int, b: Int, c: Int) {}
        fn getUser(a: Map<String, String>) -> Map<String, String> {
            let nums: Array<Int> = [1, 2, 3, 4];
            return nums, 5;
        }

        // For loops
        for i in 0..10 {
            if b > c {
                continue;
                if a > b {
                    let c = 11-2;
                    break;
                }
            }
        }

        for i in 0..=10 {

        }

        let arr = [1,2,3];
        for item in arr {

        }

        let item = {"a": 1, "b": 2};
        for key, value in item {
        }

        for {

        }

        for (key, value) in item {

        }

        for Some(x) in maybeValue {

        }

        // Print statements
        print();
        print("");
        print("Hello");
        print(5);

        print(a);
        print(myVar);
        print(a, b, "hello", 5);
        print("Value:", x);
        print(a + b);
        print(f(x, y));
        print(true);
        print(false);
        print(None);

        // Multiple assignment and underscore
        // sadkjhas
        data, info, user = GetUser(5); // newcom
        _, info = GetUser();
        x = 5;
    "#;

    // let input = fs::read_to_string("./syntax.mylang").unwrap();
    // println!("Source code:\n{}", input);
    let tokens = lex(&input);

    // for token in &tokens {
    //     println!("{:?}", token);
    // }

    // Create parser instance
    let mut parser = Parser::new(&tokens);

    // Parse the whole program
    match parser.parse_program() {
        Ok(program) => println!("{:#?}", program),
        Err(e) => eprintln!("Parse error: {:?}", e),
    }
}
