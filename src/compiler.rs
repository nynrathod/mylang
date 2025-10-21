use crate::analyzer::SemanticAnalyzer;
use crate::codegen::core::CodeGen;
use crate::diagnostics::{print_grouped, DiagnosticRecord};
use crate::lexar::lexer::lex;
use crate::mir::builder::MirBuilder;
use crate::parser::{ast::AstNode, ParseError, Parser};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub struct CompileOptions {
    pub input_path: PathBuf,
    pub output_name: String,
    pub dev_mode: bool,
    pub print_ast: bool,
    pub print_mir: bool,
    pub keep_ll: bool,
    pub check_only: bool,
    pub release_mode: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            input_path: PathBuf::from("."),
            output_name: "output".to_string(),
            dev_mode: cfg!(debug_assertions),
            print_ast: false,
            print_mir: false,
            keep_ll: false,
            check_only: false,
            release_mode: false,
        }
    }
}

pub struct CompileResult {
    pub success: bool,
    pub error_count: usize,
}

pub fn compile_project(opts: CompileOptions) -> Result<CompileResult, String> {
    // Find main.my or use custom path
    let input_path = if opts.input_path.is_file() {
        opts.input_path.clone()
    } else {
        let main_file = opts.input_path.join("main.my");
        if !main_file.exists() {
            return Err(format!(
                "Error: main.my not found in {}",
                opts.input_path.display()
            ));
        }
        main_file
    };

    let input = fs::read_to_string(&input_path)
        .map_err(|e| format!("Failed to read {}: {}", input_path.display(), e))?;

    let project_root = input_path.parent().unwrap().to_path_buf();

    // Lex
    let tokens = lex(&input);
    let mut parser = Parser::new(&tokens);
    let mut analyzer = SemanticAnalyzer::new(Some(project_root));

    let mut diagnostics: Vec<DiagnosticRecord> = Vec::new();
    let mut error_count = 0;

    // Parse
    let mut statements = Vec::new();
    while parser.current < parser.tokens.len() {
        match parser.parse_statement() {
            Ok(stmt) => statements.push(stmt),
            Err(e) => {
                let (line, col, msg) = match &e {
                    ParseError::UnexpectedTokenAt { msg, line, col } => {
                        (Some(*line), Some(*col), msg.clone())
                    }
                    _ => (None, None, e.to_string()),
                };
                diagnostics.push(DiagnosticRecord {
                    filename: input_path.display().to_string(),
                    message: msg,
                    line,
                    col,
                    is_parse: true,
                });
                skip_to_next_statement(&mut parser);
                error_count += 1;
            }
        }
    }

    // Analyze
    if let Err(e) = analyzer.analyze_program(&mut statements) {
        diagnostics.push(DiagnosticRecord {
            filename: input_path.display().to_string(),
            message: e.to_string(),
            line: None,
            col: None,
            is_parse: false,
        });
        error_count += 1;
    }

    // Print diagnostics if any
    if !diagnostics.is_empty() {
        let mut sources = HashMap::new();
        sources.insert(input_path.display().to_string(), input.clone());
        print_grouped(&diagnostics, &sources);
    }

    if error_count > 0 {
        if opts.dev_mode {
            println!("\nFound {} errors, skipping codegen", error_count);
        }
        return Ok(CompileResult {
            success: false,
            error_count,
        });
    }

    if opts.check_only {
        return Ok(CompileResult {
            success: true,
            error_count: 0,
        });
    }

    // Build MIR
    let mut all_nodes = analyzer.imported_functions.clone();
    all_nodes.extend(statements);

    if opts.print_ast {
        println!("\n=== AST ===\n{:#?}", AstNode::Program(all_nodes.clone()));
    }

    let mut mir_builder = MirBuilder::new();
    mir_builder.build_program(&all_nodes);
    mir_builder.finalize();

    if opts.print_mir || opts.dev_mode {
        println!("\n=== MIR ===\n{:#?}", mir_builder.program);
    }

    // Generate LLVM IR
    let context = inkwell::context::Context::create();
    let mut codegen = CodeGen::new("main_module", &context);
    codegen.generate_program(&mir_builder.program);

    if opts.dev_mode {
        println!("\n=== LLVM IR ===");
        codegen.dump();
    }

    let llvm_ir = codegen.module.print_to_string();

    // For devs or debugging: if keep_ll is set, write IR to file as before
    if opts.keep_ll {
        let ll_file = format!("{}.ll", opts.output_name);
        fs::write(&ll_file, llvm_ir.to_string())
            .map_err(|e| format!("Failed to write LLVM IR: {}", e))?;

        // Print the clang command for debugging
        println!("Running: clang {} -o {}", ll_file, opts.output_name);

        let mut cmd = Command::new("clang");
        cmd.arg(&ll_file).arg("-o").arg(&opts.output_name);

        if opts.release_mode {
            cmd.arg("-O2");
        }

        // Suppress clang warnings by redirecting stderr to null
        #[cfg(unix)]
        {
            use std::fs::File;
            use std::os::unix::process::CommandExt;
            let devnull = File::open("/dev/null").unwrap();
            cmd.stderr(devnull);
        }
        #[cfg(windows)]
        {
            use std::fs::OpenOptions;
            use std::os::windows::process::CommandExt;
            let devnull = OpenOptions::new().write(true).open("NUL").unwrap();
            cmd.stderr(devnull);
        }

        let output = cmd
            .output()
            .map_err(|_| "clang not found. Install clang/LLVM".to_string())?;
        if !output.status.success() {
            if output.stderr.is_empty() {
                // Print the contents of the .ll file for debugging
                let ll_contents = fs::read_to_string(&ll_file)
                    .unwrap_or_else(|_| "<could not read .ll file>".to_string());
                eprintln!("Clang failed with no error output. Please check your source code and the generated LLVM IR file '{}'.", ll_file);
                eprintln!(
                    "--- Begin .ll file ---\n{}\n--- End .ll file ---",
                    ll_contents
                );

                // Optionally, check for a main function in the IR
                if !ll_contents.contains("define") || !ll_contents.contains("main") {
                    eprintln!("Hint: The LLVM IR does not contain a 'main' function. Make sure your source code defines an entry point.");
                }
            } else {
                eprintln!("Clang stderr:\n{}", String::from_utf8_lossy(&output.stderr));
            }
            return Err("Compilation failed".to_string());
        }

        // Cleanup
        if !opts.keep_ll {
            let _ = fs::remove_file(&ll_file);
        }
    } else {
        // For end users: pipe IR directly to clang via stdin, do not write .ll file
        println!("Running: clang (via stdin) -o {}", opts.output_name);

        let mut cmd = Command::new("clang");
        cmd.arg("-x")
            .arg("ir")
            .arg("-o")
            .arg(&opts.output_name)
            .arg("-");

        if opts.release_mode {
            cmd.arg("-O2");
        }

        // Suppress clang warnings by redirecting stderr to null
        #[cfg(unix)]
        {
            use std::fs::File;
            use std::os::unix::process::CommandExt;
            let devnull = File::open("/dev/null").unwrap();
            cmd.stderr(devnull);
        }
        #[cfg(windows)]
        {
            use std::fs::OpenOptions;
            use std::os::windows::process::CommandExt;
            let devnull = OpenOptions::new().write(true).open("NUL").unwrap();
            cmd.stderr(devnull);
        }

        use std::io::Write;
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::inherit())
            .spawn()
            .map_err(|_| "clang not found. Install clang/LLVM".to_string())?;

        {
            let stdin = child
                .stdin
                .as_mut()
                .ok_or("Failed to open stdin for clang")?;
            stdin
                .write_all(llvm_ir.to_string().as_bytes())
                .map_err(|_| "Failed to write LLVM IR to clang stdin".to_string())?;
        }

        let output = child
            .wait_with_output()
            .map_err(|_| "Failed to wait for clang process".to_string())?;

        if !output.status.success() {
            if output.stderr.is_empty() {
                eprintln!("Clang failed with no error output. Please check your source code and the generated LLVM IR (piped to clang).");
                // Optionally, print the IR for debugging
                // eprintln!("--- Begin LLVM IR ---\n{}\n--- End LLVM IR ---", llvm_ir);
                let llvm_ir_str = llvm_ir.to_string();
                if !llvm_ir_str.contains("define") || !llvm_ir_str.contains("main") {
                    eprintln!("Hint: The LLVM IR does not contain a 'main' function. Make sure your source code defines an entry point.");
                }
            } else {
                eprintln!("Clang stderr:\n{}", String::from_utf8_lossy(&output.stderr));
            }
            return Err("Compilation failed".to_string());
        }
    }

    Ok(CompileResult {
        success: true,
        error_count: 0,
    })
}

fn skip_to_next_statement(parser: &mut Parser) {
    while parser.current < parser.tokens.len() {
        if let Some(tok) = parser.peek() {
            if matches!(tok.kind, crate::lexar::token::TokenType::Semi) {
                parser.advance();
                break;
            }
        }
        parser.advance();
    }
}
