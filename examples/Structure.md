## 📁 Project Structure

This is the **Example folder structure** for a typical mylang project.
It helps keep your code organized, modular, and easy to maintain.

- **main.my**: Your project entry point.
- **http/**: Example of a feature/module folder (e.g., for HTTP-related code).
- **models/**: Example of a folder for data models or business logic.

You can add more folders (e.g., `services/`, `utils/`, `tests/`) as your project grows.

```text
myproject/
├── main.my          # Entry point
├── http/
│   └── Client.my    # Module: HTTP client logic
└── models/
    └── User.my      # Module: User-related logic
```

**Notes:**
- Use PascalCase for file names that define modules with public functions (e.g., `User.my`, `Client.my`).
- Keep all related files for a feature/module in the same folder.
- Imports should use the folder and PascalCase file name, e.g., `import models::User::Usergreet;`.


```rust
    // models/User.my
    fn Usergreet(user: Str) -> Str {
        return "Hello, " + user;
    }

    // http/Client.my
    fn Fetchuser(id: Int) -> Str {
        return "Alice";
    }

    // main.my
    import http::Client::Fetchuser;
    import models::User::Usergreet;

    fn main() {
        let user = Fetchuser(1);
        let greeting = Usergreet(user);
        print(greeting);
        print(user);
    }

```
