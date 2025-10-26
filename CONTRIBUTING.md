# Contributing to Doo

Thank you for your interest in contributing to Doo! This document provides guidelines for developers who want to contribute to the Doo compiler and language development.

## 🚀 Development Setup

### Prerequisites for Development

To contribute to Doo, you'll need:

- **Rust**: Install from [rustup.rs](https://rustup.rs/)
- **System Linker**: C/C++ compiler (clang, gcc, or MSVC) for linking
- **LLVM 18.1.8**: Required for the code generation backend [Link](https://github.com/llvm/llvm-project/releases?page=4)
- **Git**: For version control

### Building from Source

```bash
# Clone the repository
git clone https://github.com/nynrathod/doolang/
cd doo

# Build in release mode (optimized)
cargo build --release --bin doo

# Run tests
cargo test
or
cargo nextest run

# Run the development compiler
cargo run
```

## 🔧 Compilation Pipeline

### Technical Overview

Doo uses a sophisticated compilation pipeline to transform source code into native executables:

### Key Components

- **Lexer**: Tokenizes source code into keywords, identifiers, and operators
- **Parser**: Builds Abstract Syntax Tree (AST) from token stream
- **Semantic Analyzer**: Performs type checking and validation
- **MIR Builder**: Generates mid-level intermediate representation
- **Code Generator**: Produces LLVM IR and coordinates native compilation

```
Source Code (.doo files)
      ↓
   Lexical Analysis (Tokenizer)
      ↓
   Syntax Analysis (Parser)
      ↓
Semantic Analysis (Type Checker)
      ↓
   MIR Generation (Mid-level IR)
      ↓
  LLVM IR Generation (Backend)
      ↓
 Object File Generation (via clang)
      ↓
   Native Linking (via lld(in Windows) and clang(In Linus and macOS) - Single executable)
```


### Compiler Options

The development tool supports various compilation modes:

- **Debug Mode**: `cargo run` - Includes debug information and prints AST/MIR
- **Release Mode**: `cargo build --release --bin doo` - Optimizes for performance


## 🛠️ Development Workflow

### 1. Fork and Clone

1. Fork the repository on GitHub
2. Clone your fork locally

### 2. Create a Feature Branch

Always create a feature branch for your work:
```bash
git checkout -b feature/amazing-feature
# or
git checkout -b fix/bug-description
```

### 3. Make Your Changes

- Follow the existing code style and patterns
- Add tests for new functionality
- Ensure all tests pass: `cargo test`
- Update documentation as needed

### 4. Test Your Changes

```bash
# Run all tests
cargo test or  cargo nextest run
```

### 5. Commit and Push
### 6. Create a Pull Request


## 🐛 Reporting Issues

### Bug Reports

When reporting bugs, please include:

1. **Doo version**: Output of `cargo --version`
2. **Operating System**: Windows/Linux/macOS and version
3. **Minimal reproduction case**: Smallest possible code example
4. **Expected vs. actual behavior**
5. **Error messages**: Copy the full error output

## 🚀 Getting Help

### Development Discussions

- **Issues**: Use GitHub issues for bug reports and feature requests
- **Discussions**: Use GitHub discussions for questions and ideas
- **Pull Requests**: Welcome for all types of contributions

## 📜 License

By contributing, you agree that your contributions will be licensed under the same license as the original project.

---

Thank you for contributing to Doo! 🎉
