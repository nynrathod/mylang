

## 🚀 Quick Start Commands

### Run All Tests
```powershell
cargo test
```

### Run Specific Stage
```powershell
cargo test --lib lexer_tests      # Lexer only
cargo test --lib parser_tests     # Parser only
cargo test --lib analyzer_tests   # Analyzer only
cargo test --lib mir_tests        # MIR only
cargo test --lib codegen_tests    # Codegen only
cargo test --test integration_tests  # Integration only
```

### Verbose Output
```powershell
cargo test -- --nocapture
```

### Single Test
```powershell
cargo test test_basic_tokens
```

# Show output for specific test
```powershell
cargo test test_basic_tokens -- --nocapture
```

### Verbose Mode
```powershell
cargo test -- --test-threads=1 --nocapture
```


## Adding New Tests

### Example: Add a new parser test
```rust
#[test]
fn test_my_feature() {
    let input = "your code here";
    let tokens = lex(input);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse_statement();
    assert!(result.is_ok());
}
```

### Example: Add a new analyzer test
```rust
#[test]
fn test_my_semantic_check() {
    let input = "fn main() { /* your code */ }";
    assert!(analyze_code(input).is_ok());
}
```
