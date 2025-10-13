## 📁 Project Structure

```text
myproject/
├── main.my
├── lib.my
├── http/
│   └── client.my
└── models/
    └── user.my
```

```rust
 // models/user.my
    struct User {
        Name: Str,
        Email: Str,
        age: Int,  // private
    }
    fn Create_user(name: Str, email: Str) -> User { User { name, email, age: 0 } }
    fn internal_helper() {}  // private

    // http/client.my
    import models::user::User
    fn fetch_user(id: Int) -> (User, Str) {}

    // main.my
    import http::client::fetch_user
    import models::user::User
    fn main() {
        let user = fetch_user(1)?;
        print(user.name);
    }
```
