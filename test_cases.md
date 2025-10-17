// fn VarDecl() {
//     // ====== Variable - Valid
//     let mut aa = 1 < 2;
//     let mut asd = aa;
//     let a: Int = 42 + 5;
//     // TODO: check codgen for string concat
//     let b: Str = "hello" + "asdas";
//     let bty = b;
//     // let bb: Str = "hello" + " qwewasdas";
//     let c: Bool = false;
//     let mut b1 = "s";
//     let mut a1 = "s";
//     let mut x1: Int = 0;
//     let mut y1: Str = "";

//     let mut z1: Int = 0;
//     let mut d: [Int] = [1, 2, 3];
//     let mut e: {Str: Int} = {"a": 1, "b": 2};

//     // // ====== Variable RC
//     let aRc = {"SAd": 5, "ASds": 8, "ASdfs": 10};
//     let bRc = 10;
//     let n1 = 5;
//     n1 = 10;

//     let s1: Str = "hello";
//     let s2 = s1; // RC increment s2 only
//     let mut s3 = "world";
//     s3 = "asdas";
//     let s4 = s3; // RC increment s4 only

//     let i1: Int = 10;
//     let i2 = i1;
//     let b1Rc: Bool = true;
//     let b2 = b1Rc;

//     let arr1: [Int] = [1, 2, 3];
//     let arr2 = arr1; // RC increment outer array

//     let arrS1: [Str] = ["a", "b"];
//     let arrS2 = arrS1; // RC outer array + inner strings

//     let map1: {Str: Int} = {"a":1, "b":2};
//     let map2 = map1;      // RC increment map

//     let cond: Bool = true;

//     let arrLoop: [Str] = ["a","b","c"];
//     let mapLoop: {Str: Str} = {"k1":"v1", "k2":"v2"};

//     let s7: Str = "copyTest";
//     let s8 = s7;
//     let s9 = s8;

//     let arrCopy1 = arrS1;
//     let arrCopy2 = arrCopy1;
// }

// fn ForLoops() {
//     // ====== Range For Loops ======

//     // Basic range loop (exclusive)
//     for i in 0..5 {
//         // Loop variable i goes from 0 to 4
//     }

//     // Range with variable end
//     let n = 10;
//     for i in 0..n {
//         // Loop from 0 to n-1
//     }

//     // Inclusive range loop
//     for i in 0..=5 {
//         // Loop variable i goes from 0 to 5 (inclusive)
//     }

//     // ====== Array For Loops ======

//     // Integer array iteration
//     let arr: [Int] = [1, 2, 3, 4, 5];
//     for item in arr {
//         // item is each integer from the array
//         // No RC needed for integers
//     }

//     // String array iteration (RC needed)
//     let strarr: [Str] = ["hello", "world", "test"];
//     for stritem in strarr {
//         // str_item gets RC increment when loaded
//         // Automatic RC decrement at end of iteration
//     }

//     // ====== Map For Loops ======

//     // String to Int map
//     let map1: {Str: Int} = {"apple": 1, "banana": 2, "cherry": 3};
//     for (key, value) in map1 {
//         // key gets RC increment (string)
//         // value is int (no RC needed)
//         // Automatic cleanup at end of iteration
//     }

//     // String to String map (both need RC)
//     let map2: {Str: Str} = {"hello": "world", "foo": "bar"};
//     for (k, v) in map2 {
//         // Both k and v get RC increment
//         // Both cleaned up at end of iteration
//     }

//     // ====== Nested Loops with RC ======

//     let outerarr: [Str] = ["a", "b"];
//     for outeritem in outerarr {
//         // outer_item gets RC
//         let innerarr: [Str] = ["x", "y"];
//         for inneritem in innerarr {
//             // inner_item gets RC
//             // Proper nested cleanup
//         }
//         // inner_item cleaned up here
//         // inner_arr cleaned up here
//     }
//     // outer_item cleaned up here
//     // outer_arr cleaned up here

//     // ====== Break and Continue with RC ======

//     let testarr: [Str] = ["one", "two", "three", "four"];
//     for item in testarr {
//         // item gets RC increment

//         // Simulate condition for break
//         let shouldbreak = false;
//         if shouldbreak {
//             // RC cleanup happens before break
//             break;
//         }

//         // Simulate condition for continue
//         let shouldcontinue = false;
//         if shouldcontinue {
//             // RC cleanup happens before continue
//             continue;
//         }

//         // Normal iteration continues
//     }
//     // Final cleanup happens here

//     // ====== Infinite Loop ======

//     let counter = 0;
//     for {
//         // Infinite loop
//         counter = counter + 1;

//         // Simulate break condition
//         if counter > 5 {
//             break;
//         }
//     }

//     // ====== Complex RC Scenarios ======

//     // Array of arrays (nested RC)
//     let nested: [[Str]] = [["a", "b"], ["c", "d"]];
//     for subarray in nested {
//         // sub_array gets RC
//         for strelement in subarray {
//             // str_element gets RC
//             // Proper nested RC management
//         }
//     }

//     // Variable assignment in loops
//     let result: Str = "";
//     let sources: [Str] = ["hello", "world"];
//     for source in sources {
//         // source gets RC
//         result = source; // result gets RC, old value decremented
//     }
//     // Final cleanup of all variables
// }

// fn ConditonalTest() {
//     // ====== Conditional - Valid
//     let conditionA: Bool = true;
//     if conditionA {
//         let x2: Int = 10;
//         print(x2);
//     } else {
//         let y2: Str = "ok";
//         y2 = "adas";
//         print(y2);
//     }

//     let conditionB: Bool = true;
//     let conditionC: Bool = false;
//     if conditionB {
//         if conditionC {
//             let n: Int = 123;
//             print(n);
//         } else {
//             let arr: [Int] = [1, 2, 3];
//             print(arr);
//         }
//     }

//     let conditionD: Bool = true;
//     if conditionD {
//         let msg: Str = "hello";
//         print(msg);
//     } else {
//         let m: {Str: Int} = {"a": 1, "b": 2};
//         print(m);
//     }

//     let conditionE: Bool = true;
//     let conditionF: Bool = false;
//     for i in [10, 20, 30] {
//         if i == 20 {
//             print("continue!");
//             continue;
//         } else if i == 30 {
//             print("break!");
//             break;
//         }
//         print(i);
//     }

//     if conditionE {
//         let u: Int = 10;
//         print(u);
//     } else if conditionF {
//         let v: Int = 20;
//         print(v);
//     } else {
//         let w: Int = 30;
//         print(w);
//     }
// }

// // ====== ASSIGNMENTS - VALID
// fn getValue1() -> Int {
//     let a = 55;
//     return a;
// }

// fn getValue() -> Int {
//     return 5;
// }

// fn getStr() -> Str {
//     return "hello";
// }

// fn makeArray() -> [Int] {
//     return [1, 2, 3];
// }

// fn makeStrArray() -> [Str] {
//     return ["a", "b"];
// }

// fn makeMap() -> {Str: Int} {
//     return {"a": 1, "b": 2};
// }

// fn mathOperation(x: Int, y: Int) -> Int {
//     return 11;
// }

// fn doSomething(a: Str) -> Str {
//     let mut result = "hello";
//     return "asd";
// }

// fn testFunctionCalls() {
//     // Int return assignments
//     let a2 = getValue();
//     let a: Int = getValue();
//     let b = getValue();

//     let mat = mathOperation(2, 3);
//     let doS = doSomething("world");

//     // String return assignments
//     let s: Str = getStr();
//     let s2 = getStr();

//     // Array[Int] return assignments
//     let arr: [Int] = makeArray();
//     let arr2 = makeArray();

//     // Array[Str] return assignments
//     let arrS: [Str] = makeStrArray();
//     let arrS2 = makeStrArray();

//     // Map return assignments
//     let m: {Str: Int} = makeMap();
//     let m2 = makeMap();

// }

// fn ValidCaseFunc() {

// // ====== FUNCTIONS - VALID
// fn getValue() -> Int {
//     return 5;
// }

// fn simpleFunction(param: Str) -> Str {
//     let aa = return param;
// }
// fn mathOperation(x: Int, y: Int) -> Int {
//     return x + y;
// }
// fn arrayFunction(arr: [Int]) -> [Int] {
//     return arr;
// }

// fn conditionalReturn() -> Int {
//     if true {
//         return 1;
//     } else {
//         return 2;
//     }
// }
// fn scopeTest() {
//     let localVar: Int = 5;
//     print(localVar);
// }
// fn validFunction(param: Int) {
//     let localB: Int = 99;
// }
// fn overload1(param: Int) -> Int {
//     return param;
// }
// fn overload2(param: Str) -> Str {
//     return param;
// }

// // ====== PRINT STATEMENTS - VALID CASES
fn abc() {
    for i in 0..100 {
        print(42);
    }
}

fn main() {
    let user = "ASdas";
    // let user = "ASdasd";
    abc(); // Just call, no assignment
}

// print(42);
// print(true);
// print("Hello World");
// let x: Int = 10;
// print(x);
// let y: Int = 30;
// print("x:", x, "y:", y);
// print(x + y, "sum:", x + y);
// let arr1 = [1, 2, 3];
// print(arr1);
// let arr2 = ["a", "b", "c"];
// print(arr2);
// let m1 = { "a": 1, "b": 2 };
// print(m1);
// let m2 = { "num": "x", "str": "hi" };
// print(m2);
// print(x + y);
// print(x > y);
// print("result:", x + y);

// // ====== STRUCTS AND ENUMS - VALID
// struct FAuserProfile {
//     name: Str,
//     age: Int,
// }

// struct FAadminRole {
//     level: Int
// }

// struct FAmixedTypes {
//     id: Int,
//     username: Str,
//     isActive: Bool,
// }

// struct FAcompany {
//     name: Str,
//     ceo: FAuserProfile,
// }

// struct FAcontainer {
//     user: FAuserProfile,
//     role: FAuserRole,
// }

// struct FAemptyStruct {}

// enum FAuserRole {
//     Admin(FAadminRole),
//     Guest,
//     Moderator,
// }

// enum FAoptionExample {
//     Somess(Int),
//     None,
// }

// enum FAsimpleEnum {
//     Red,
//     Green,
//     Blue,
// }

// enum FAemptyEnum {}

// }

// fn InValidCaseFunc() {

// // ====== VARIABLE - INVALID
// let FAa: Str = 5;
// let FAb: Int = true;
// let FAc: {Str, Int} = [1, 2, 3];
// let FAd: {Str} = "hello";

// let FAe: Int = 10;
// let FAe: Int = 20;

// let FAf: Str = "abc";
// let FAf: Int = 123;

// let FAg: Int = FAunknown;
// let FAh: Str = FAmissing;

// // ====== Conditional - INVALID
// let FAcondition: Int = 5;
// if FAcondition {
//     let FAwrong: Int = 99;
// }

// if FAundeclared {
//     let FAc: Int = 1;
// }

// let FAd: Bool = true;
// if FAd {
//     let FAe: Int = FAunknownvar;
// }

// let FAf: Bool = true;
// if FAf {
//     let FAg: Int = 1;
// } else {
//     let FAh: Str = FAmissing;
// }

// let FAi: Bool = true;
// if FAi {
//     let FAj: Int = "oops";
// } else {
//     let FAk: Str = 42;
// }

// let FAo: Bool = true;
// let FAp: Int = 42;
// if FAo {
//     if FAp {
//         let FAq: Int = 99;
//     }
// }

// // ====== FUNCTIONS - INVALID
// fn FAfoo() {
//     return 42;
// }
// fn FAwrongReturn2() -> Int { return "hi"; }
// fn FAwrongReturn3() { return 5; }
// fn FAwrongReturn4() -> Int {
//     if true { return 1; }
//     else { return "oops"; }
// }
// fn FAdup(param: Int) -> Int { return param; }
// fn FAdup(param: Int) -> Str { return param; }
// fn FAdupParam(param: Int, param: Int) {}
// fn FAuseUndeclared() {
//     print(FAundeclared);
// }

// // ASSIGNMENTS - INVALID
// let FAa, FAb = 42;
// let FAa, FAb = "invalid";

// fn FAsomeFunction5(a: Int, b: Int) -> Int { return 5; }
// let FAd, FAc = FAsomeFunction5(1, 2);

// // ====== PRINT STATEMENTS - INVALID
// print();
// print(x y);
// print("Hello" "World");
// print(x + );

// // ====== FOR LOOPS - INVALID
// let FAmap = {"a": 1, "b": 2};
// for FAkey in FAmap {
//     print(FAkey);
// }

// for (FAk) in FAmap {
//     print(FAk);
// }
// let FAarr = [1, 2, 3];
// for (FAx, FAy) in FAarr {
//     print(FAx, FAy);
// }

// let FAmaybeValue = "20";
// for FAx in FAmaybeValue {
//     print(FAx);
// }

// let FAnum = 42;
// for FAx in FAnum {
//     print(FAx);
// }

// let FAtuplearr = {"a": 1, "b": 2};
// for (FAa, FAb, FAc) in FAtuplearr {
//     print(FAa, FAb, FAc);
// }

// for FAi in 10 {
//     print(FAi);
// }

// let FAmaybeBool = true;
// for FAb in FAmaybeBool {
//     print(FAb);
// }

// // ====== STRUCTS AND ENUMS - INVALID
// struct FAinvalidStruct {
//     FAname: Str,
//     FAname: Int,
// }

// enum FAinvalidEnum {
//     FAFirst,
//     FASecond,
//     FAFirst,
// }

// enum FAinvalidEnum2 {
//     FAuserProfile,
//     FAguest,
// }

// }
