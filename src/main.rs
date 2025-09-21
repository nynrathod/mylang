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
    //      let z;

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

    // let input = r#"
    //     let b = "s";
    //     let mut a = "s";
    //     // Valid cases
    //     // fn GetValue() -> Int{ return 5; }
    //     // a = GetValue();

    //     // Multiple variables from function returning tuple
    //     // a, b = GetUser(5);

    //     // Multiple variables with wildcards
    //     // a, _, c = GetUser(5);

    //     // fn SomeFunction2(a: Int, b: Int) -> (Int, String) { return 5, "s"; }
    //     // a, b = SomeFunction2(1, 2);

    //     // fn SomeFunction(a: Int, b: Int) -> (Int, String) { return 5, "s"; }
    //     // let mut a, _ = SomeFunction(1, 2);

    //     // fn SomeFunction8(a: Int, b: Int) -> (Int, String, String) { return 5, "s", "sad"; }
    //     // let  (a, (b, _)) = SomeFunction8(1, 2);

    //     // invalid

    //     // wrong function
    //     // a, b = 42;
    //     // a, b = "sdas";

    //      // function not declared
    //     // a, b = UnknownFunc(5);

    //     // fn SomeFunction3(a: Int, b: Int) -> (Int, String) { return 5, "s"; }
    //     // a = SomeFunction3(1, 2);

    //     // fn SomeFunction4(a: Int, b: Int) -> (Int, String) { return 5, "s"; }
    //     // a, b = SomeFunction4(1);

    //     // fn SomeFunction6(a: Int, b: Int) -> (Int, String) { return 5, "s"; }
    //     // a, b = SomeFunction6(1, "Sda");

    //     // fn SomeFunction5(a: Int, b: Int) -> Int { return 5; }
    //     // let d, c = SomeFunction5(1, 2);

    //     // if, else = GetUser(5);
    //     "#;

    // let input = r#"

    //     //Single values
    //     // print(42);

    //     // // print(true);
    //     // print("Hello World");
    //     // let x: Int = 10;
    //     // print(x);

    //     // // Multiple items
    //     // let y: Int = 30;
    //     // print("x:", x, "y:", y);
    //     // print(x + y, "sum:", x + y);

    //     // // Arrays / Lists
    //     // let arr1 = [1, 2, 3];
    //     // print(arr1);
    //     // let arr2 = ["a", "b", "c"];
    //     // print(arr2);
    //     // print([x, y, x+y]);

    //     // // Maps / Dictionaries
    //     // let m1 = { "a": 1, "b": 2 };
    //     // print(m1);
    //     // let m2 = { "num": x, "str": "hi" };
    //     // print(m2);
    //     // print({ "sum": x+y });

    //     // // Nested structures
    //     // print([[1,2],[3,4]]);
    //     // print({ "nums": [1,2,3] });
    //     // print({ "user": { "name": "Alice", "age": 25 } });

    //     // // Expressions directly
    //     // print(x + y);
    //     // print(x > y);
    //     // print("result:", x + y);

    //     // Unsupported types
    //     // let f: Int = fn() {};
    //     // print(f);

    //     // Incorrect usage
    //     // print();
    //     // print(x y);
    //     // print("Hello" "World");
    //     // print(x + );

    //     "#;

    // let input = r#"

    //         // 	// Array of arrays of Int
    //         // let matrix: [[Int]] = [[1, 2], [3, 4], [5, 6]];

    //         // // Map from Str to array of Int
    //         // let stats: {Str, [Int]} = {"scores": [10, 20, 30], "levels": [1, 2, 3]};

    //         // // Array of maps from Str to Int
    //         // let users: [{Str, Int}] = [
    //         //     {"id": 1, "age": 25},
    //         //     {"id": 2, "age": 30}
    //         // ];

    //         // // Map from Str to array of maps from Str to Int
    //         // let userGroups: {Str, [{Str, Int}]} = {
    //         //     "admins": [
    //         //         {"id": 1, "age": 25},
    //         //         {"id": 2, "age": 30}
    //         //     ],
    //         //     "guests": [
    //         //         {"id": 3, "age": 22}
    //         //     ]
    //         // };

    //         // // Array of array of maps from Str to Int
    //         // let deepNested: [[{Str, Int}]] = [
    //         //     [
    //         //         {"id": 1, "score": 100},
    //         //         {"id": 2, "score": 90}
    //         //     ],
    //         //     [
    //         //         {"id": 3, "score": 80}
    //         //     ]
    //         // ];

    //         //    let mapmixedkeys = {1: "a", 2: "b"};
    //         //    let nestedmapmixed = {
    //         //        "a": {1: "x"}, // Inner map key type conflict
    //         //        "b": {2: "y"}
    //         //    };

    //         // Mixing Int and String
    //         // Semantic error: VarTypeMismatch(TypeMismatch { expected: Int, found: String })
    //         // let arrmixed = [1, "2", 3];

    //         // Empty array, type can't be inferred
    //         // Semantic error: EmptyCollectionTypeInferenceError(TypeMismatch { expected: Array(Int), found: Array(Void) })
    //         // let arrempty: [Int] = [];

    //         // Semantic error: VarTypeMismatch(TypeMismatch { expected: Array(Int), found: Array(String) })
    //         // let nestedmixed = [[1, 2], ["a", "b"]]; // Array of arrays, inner arrays have different types

    //         // // --- Map Errors ---
    //             // Keys not same type (Int vs String)
    //             // Semantic error: VarTypeMismatch(TypeMismatch { expected: Int, found: String })
    //         // let mapmixedvalues = {"a": 1, "b": "2"}; // Values not same type

    //         // Empty map, can't infer types
    //         // Semantic error: EmptyCollectionTypeInferenceError(TypeMismatch { expected: Map(String, Int), found: Map(Void, Void) })
    //         // let mapempty: {Str, Int} = {};

    //         // // --- Map of arrays ---
    //         // Semantic error: VarTypeMismatch(TypeMismatch { expected: Array(Int), found: Array(String) })
    //         // let mapofarraysmixed = {
    //         //     "nums": [1, 2],
    //         //     "letters": ["a", "b"] // value type mismatch
    //         // };

    //         // // --- Deeply nested errors ---
    //         // Semantic error: VarTypeMismatch(TypeMismatch { expected: Int, found: String })
    //         // let deepnestedinvalid: [[{Str, Int}]] = [
    //         //     [
    //         //         {"id": 1, "score": 100},
    //         //         {"id": 2, "score": "high"} // value type mismatch in nested map
    //         //     ]
    //         // ];

    //         // Semantic error: InvalidMapKeyType { found: Array(String), expected: [Int, String, Bool] }
    //         // let mapwitharraykeys = {
    //         //     ["a", "b"]: [1, 2],     // Array as key is invalid if not allowed
    //         //     ["c"]: [3]
    //         // };

    //         // // Map from array of Str to array of Int
    //         // Semantic error: InvalidMapKeyType { found: Array(String), expected: Map(Int, Void) }
    //         // let complexMap: {[Str], [Int]} = {
    //         //     ["a", "b"]: [1, 2],

    //         //     ["c"]: [3]
    //         // };

    //         // // --- Array of maps --- to test later
    //         // Semantic error: VarTypeMismatch(TypeMismatch { expected: String, found: Int })
    //         // let arrofmapsmixed = [
    //         //     {"id": 1, "age": 25},
    //         //     {"id": "x", "age": 30} // id type mismatch
    //         // ];
    // "#;

    let input = r#"



        let n = 5;
     //    for i in 0..n {
    	// }

	    // for i in 0..10 {
			  // // 0 to 9
     //           print(i);
     //       }

     //       for i in 0..=10 {
     //            // 0 to 10 (inclusive)
     //           print(i);
     //       }

     //       let arr = [1,2,3];
     //       for item in arr {
     //           print(item);
     //       }

     //       let map = {"a": 1, "b": 2};
     //       for (key, value) in map {
     //           print(key, value);
     //       }

     //       for {
     //           print("running forever");
     //       }

     //       for (key, value) in map {
     //           print(key, value);
     //       }






     // Map iteration with wrong pattern
         // INVALID: must destructure as (key, value)
     let map = {"a": 1, "b": 2};
     //     for key in map {
     //         print(key);
     //     }

         // Tuple pattern mismatch in map
         // for (k) in map {
         //     print(k);
         // }

         // Array iteration with tuple pattern
         // let arr = [1, 2, 3];
         // for (x, y) in arr {
         //     print(x, y);
         // }



         // // Iterating non-iterable type
         // let maybeValue = "20";
         // for x in maybeValue {
         //     print(x);
         // }

         // let num = 42;
         // for x in num {
         //     print(x); // INVALID: Int is not iterable
         // }

         // // Tuple pattern length mismatch
         // let tuplearr = {"a": 1, "b": 2};
         // for (a, b, c) in tuplearr {
         //     print(a, b, c); // INVALID: tuple length 2 but pattern expects 3
         // }

         // // Using literal directly without range
         // for i in 10 {
         //     print(i); // INVALID: literal not iterable
         // }

         // // Using expression that is not iterable
         // let maybeBool = true;
         // for b in maybeBool {
         //     print(b); // INVALID: Bool not iterable
         // }


         // // Nested redeclaration in same loop body (if strict)
         // for x in arr {
         //     let x = 10; // INVALID in strict mode: variable redeclared inside same scope
         //     print(x);
         // }


        // let maybeValue = "20";
        //    for Some(x) in maybeValue {
        //        print("found", x);
        //    }

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
