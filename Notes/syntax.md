```rust

// PascalCase import
import crypto                  // full module
import crypto::{sha256, sha512}  // selective import
import crypto::{sha256, sha512} as c  // aliasing optional


struct UserProfile {
    name: String,
    age: Int,
}

struct AdminRole {
    level: Int
}

enum UserRole {
    Admin(AdminRole),
    Guest,
}

fn main() {

	// Basic syntax

	// # Array
	let nums: [Int] = [1, 2, 3, 4];

	// # Map
	let	user: {Str: Str} = {"name": "Alice", "role": "admin"};

	// Explicit type
	let userName: Str = "Nayan";
	let userData: [Str] = ["Nayan", "Rathod"];

	// fuction call
	let data, info = GetUser();
	let mut _, info = GetUser();

	for i in 0..n {}
	for i in 0..10 {
	  // 0 to 9
        print(i);
    }
    for i in 0..=10 {
         // 0 to 10 (inclusive)
        print(i);
    }
    let arr = [1,2,3];
    for item in arr {
        print(item);
    }
    let map = {"a": 1, "b": 2};
    for key, value in map { print(key, value); }
    for { print("running forever"); }

	if x > 10 {
	    print(">10");
	} else if x == 10 {
	    print("=10");
	} else {
	    print("<10");
	}

    // # Private Function (PascalCase)
    fn GetUser() { }

    // # Private Function (camelCase)
    fn getUser() { }

    // function with return type
    fn GetValue() -> Int { return 5; }

}
```
