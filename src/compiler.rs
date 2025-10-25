// This file contains the main compilation pipeline for the wow CLI.
// It parses, analyzes, generates LLVM IR, emits an object file, and links
// to a native executable using the system linker (clang/gcc/lld-link).
// No clang or Rust is needed for end users to run the final binary.

use crate::analyzer::types::SemanticError;
use crate::analyzer::SemanticAnalyzer;
use crate::codegen::core::CodeGen;
use crate::diagnostics::{print_grouped, DiagnosticRecord};
use crate::lexar::lexer::lex;
use crate::mir::builder::MirBuilder;
use crate::parser::{ast::AstNode, ParseError, Parser};
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::OptimizationLevel;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

// Embed linker binaries at compile time
#[cfg(target_os = "windows")]
const EMBEDDED_LINKER: &[u8] = include_bytes!("../linkers/lld-link.exe");

#[cfg(target_os = "linux")]
const EMBEDDED_LINKER: &[u8] = include_bytes!("../linkers/ld.lld");

#[cfg(target_os = "macos")]
const EMBEDDED_LINKER: &[u8] = include_bytes!("../linkers/ld64.lld");

/// Extracts the embedded linker to a temp location and returns its path
fn extract_embedded_linker() -> Result<PathBuf, String> {
    let temp_dir = env::temp_dir();

    #[cfg(target_os = "windows")]
    let linker_name = "lld-link.exe";

    #[cfg(target_os = "linux")]
    let linker_name = "ld.lld";

    #[cfg(target_os = "macos")]
    let linker_name = "ld64.lld";

    let linker_path = temp_dir.join(format!("mylang_{}", linker_name));

    // Only write if doesn't exist or size is different (updated version)
    let should_write = if linker_path.exists() {
        fs::metadata(&linker_path)
            .map(|m| m.len() != EMBEDDED_LINKER.len() as u64)
            .unwrap_or(true)
    } else {
        true
    };

    if should_write {
        let mut file = fs::File::create(&linker_path)
            .map_err(|e| format!("Failed to create linker file: {}", e))?;
        file.write_all(EMBEDDED_LINKER)
            .map_err(|e| format!("Failed to write linker: {}", e))?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&linker_path, fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("Failed to set linker permissions: {}", e))?;
        }
    }

    Ok(linker_path)
}

/// Options for controlling the compilation process.
/// These are set by the CLI and control input/output, debug, and build mode.
pub struct CompileOptions {
    /// Path to the user's project or main.my file
    pub input_path: PathBuf,
    /// Name of the output binary (no extension)
    pub output_name: String,
    /// Enable developer mode (prints extra debug info)
    pub dev_mode: bool,
    /// Print the AST after parsing
    pub print_ast: bool,
    /// Print the MIR after lowering
    pub print_mir: bool,
    /// Keep the generated LLVM IR (.ll) file
    pub keep_ll: bool,
    /// Keep the generated object (.o) file
    pub keep_obj: bool,
    /// Only check for errors, do not build
    pub check_only: bool,
}

impl Default for CompileOptions {
    /// Provides default options for compilation.
    fn default() -> Self {
        Self {
            input_path: PathBuf::from("."),
            output_name: "output".to_string(),
            dev_mode: cfg!(debug_assertions),
            print_ast: false,
            print_mir: false,
            keep_ll: false,
            keep_obj: false,
            check_only: false,
        }
    }
}

/// Result of a compilation, including success and error count.
pub struct CompileResult {
    pub success: bool,
    pub error_count: usize,
    /// Path to the generated executable (if successful)
    pub exe_path: Option<PathBuf>,
}

/// The main entry point for compiling a user project.
/// This function orchestrates the entire pipeline:
/// 1. Loads and parses the user's source file
/// 2. Performs semantic analysis and error checking
/// 3. Lowers to MIR (mid-level IR)
/// 4. Generates LLVM IR and emits an object file
/// 5. Links to a native executable using the system linker (clang/gcc/lld-link)
/// Returns a CompileResult indicating success or error count.
pub fn compile_project(opts: CompileOptions) -> Result<CompileResult, String> {
    // Check for environment variable overrides from wow CLI
    let output_name = env::var("MYLANG_OUTPUT_NAME").unwrap_or(opts.output_name);
    let check_only = env::var("MYLANG_CHECK_ONLY").is_ok() || opts.check_only;

    let opts = CompileOptions {
        output_name,
        check_only,
        ..opts
    };

    // === 1. Find and load main.my ===
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

    // === 2. Read source code ===
    let input = fs::read_to_string(&input_path)
        .map_err(|e| format!("Failed to read {}: {}", input_path.display(), e))?;

    let project_root = input_path.parent().unwrap().to_path_buf();

    // === 3. Lexing and Parsing ===
    let tokens = lex(&input);
    let mut parser = Parser::new(&tokens);
    let mut analyzer = SemanticAnalyzer::new(Some(project_root.clone()));

    let mut diagnostics: Vec<DiagnosticRecord> = Vec::new();
    let mut error_count = 0;
    let mut sources = HashMap::new();

    // Parse statements from tokens
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

    // === 4. Semantic Analysis ===
    // Create a fresh analyzer for semantic analysis
    let mut analyzer = SemanticAnalyzer::new(Some(project_root.clone()));

    if let Err(e) = analyzer.analyze_program(&mut statements) {
        use crate::analyzer::types::SemanticError;
        match &e {
            SemanticError::ParseErrorInModule { file, error } => {
                // Use regex to extract line, col, and message from error string like:
                // "parse error at 6:7: Expected OpenParen, got Semi (";")"
                let re = Regex::new(r"at (\d+):(\d+): (.+)").unwrap();
                let (line, col, msg) = if let Some(caps) = re.captures(error) {
                    (
                        caps.get(1).and_then(|m| m.as_str().parse().ok()),
                        caps.get(2).and_then(|m| m.as_str().parse().ok()),
                        caps.get(3)
                            .map(|m| m.as_str().to_string())
                            .unwrap_or_else(|| error.clone()),
                    )
                } else {
                    (None, None, error.clone())
                };
                diagnostics.push(DiagnosticRecord {
                    filename: file.clone(),
                    message: msg,
                    line,
                    col,
                    is_parse: true,
                });
                // Try to load the imported file source for snippet display
                if !sources.contains_key(file) {
                    if let Ok(src) = std::fs::read_to_string(file) {
                        sources.insert(file.clone(), src);
                    }
                }
                error_count += 1;
            }
            _ => {
                diagnostics.push(DiagnosticRecord {
                    filename: input_path.display().to_string(),
                    message: e.to_string(),
                    line: None,
                    col: None,
                    is_parse: false,
                });
                error_count += 1;
            }
        }
    }

    // Also check for any additional errors collected by the analyzer
    for error in &analyzer.collected_errors {
        match error {
            SemanticError::ParseErrorInModule {
                file,
                error: err_msg,
            } => {
                let re = Regex::new(r"at (\d+):(\d+): (.+)").unwrap();
                let (line, col, msg) = if let Some(caps) = re.captures(err_msg) {
                    (
                        caps.get(1).and_then(|m| m.as_str().parse().ok()),
                        caps.get(2).and_then(|m| m.as_str().parse().ok()),
                        caps.get(3)
                            .map(|m| m.as_str().to_string())
                            .unwrap_or_else(|| err_msg.clone()),
                    )
                } else {
                    (None, None, err_msg.clone())
                };
                diagnostics.push(DiagnosticRecord {
                    filename: file.clone(),
                    message: msg,
                    line,
                    col,
                    is_parse: true,
                });
                if !sources.contains_key(file) {
                    if let Ok(src) = std::fs::read_to_string(file) {
                        sources.insert(file.clone(), src);
                    }
                }
                error_count += 1;
            }
            _ => {
                diagnostics.push(DiagnosticRecord {
                    filename: input_path.display().to_string(),
                    message: error.to_string(),
                    line: None,
                    col: None,
                    is_parse: false,
                });
                error_count += 1;
            }
        }
    }

    // Print diagnostics if any errors found
    if !diagnostics.is_empty() {
        // Always include main file source
        sources.insert(input_path.display().to_string(), input.clone());
        // Also try to load sources for any other files in diagnostics
        for diag in &diagnostics {
            if !sources.contains_key(&diag.filename) {
                if let Ok(src) = std::fs::read_to_string(&diag.filename) {
                    sources.insert(diag.filename.clone(), src);
                }
            }
        }
        print_grouped(&diagnostics, &sources);
    }

    // Abort if errors found
    if error_count > 0 {
        if opts.dev_mode {}
        return Ok(CompileResult {
            success: false,
            error_count,
            exe_path: None,
        });
    }

    // Only check mode: skip codegen
    if opts.check_only {
        return Ok(CompileResult {
            success: error_count == 0,
            error_count,
            exe_path: None,
        });
    }

    // === 5. Lower to MIR (Mid-level IR) ===
    let mut all_nodes = analyzer.imported_functions.clone();
    all_nodes.extend(statements);

    if opts.print_ast {}

    let mut mir_builder = MirBuilder::new();
    mir_builder.build_program(&all_nodes);
    mir_builder.finalize();

    if opts.print_mir || opts.dev_mode {}

    // === 6. Generate LLVM IR and emit object file ===
    let context = inkwell::context::Context::create();
    let mut codegen = CodeGen::new("main_module", &context);
    codegen.generate_program(&mir_builder.program);

    if opts.dev_mode {
        codegen.dump();
    }

    // Only save .ll file if keep_ll is true (not just dev_mode)
    if opts.keep_ll {
        let llvm_ir = codegen.module.print_to_string();
        let ll_file = format!("{}.ll", opts.output_name);
        fs::write(&ll_file, llvm_ir.to_string())
            .map_err(|e| format!("Failed to write LLVM IR: {}", e))?;
    }

    // === 7. Native compilation and linking ===
    // Use current directory for output (where the command is run from)
    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

    let exe_name = if cfg!(windows) {
        format!("{}.exe", opts.output_name)
    } else {
        opts.output_name.clone()
    };
    let exe_path = current_dir.join(&exe_name);

    compile_to_native(&codegen, &opts, &exe_path)?;

    // After linking, check if exe exists
    if !exe_path.exists() {
        return Ok(CompileResult {
            success: false,
            error_count: 0,
            exe_path: None,
        });
    } else {
    }

    Ok(CompileResult {
        success: true,
        error_count: 0,
        exe_path: Some(exe_path),
    })
}

/// Compiles LLVM IR to a native object file and links it to a native executable.
/// Uses the system linker (clang/gcc/lld-link) for portability.
/// Cleans up the object file unless keep_obj is set.
fn compile_to_native(
    codegen: &CodeGen,
    opts: &CompileOptions,
    exe_path: &Path,
) -> Result<(), String> {
    // 1. Initialize the native target for codegen
    Target::initialize_native(&InitializationConfig::default())
        .map_err(|e| format!("Failed to initialize target: {}", e))?;

    let triple = TargetMachine::get_default_triple();
    let cpu = TargetMachine::get_host_cpu_name().to_string();
    let features = TargetMachine::get_host_cpu_features().to_string();

    let target =
        Target::from_triple(&triple).map_err(|e| format!("Failed to create target: {}", e))?;

    let opt_level = OptimizationLevel::Aggressive;

    let reloc = RelocMode::PIC;
    let model = CodeModel::Default;

    let target_machine = target
        .create_target_machine(&triple, &cpu, &features, opt_level, reloc, model)
        .ok_or("Failed to create target machine")?;

    // 2. Emit object file to current directory
    let obj_file = format!("{}.o", opts.output_name);
    target_machine
        .write_to_file(&codegen.module, FileType::Object, Path::new(&obj_file))
        .map_err(|e| format!("Failed to write object file: {}", e))?;

    // 3. Link object file to native executable
    let exe_name = exe_path.to_str().unwrap();

    link_object_file(&obj_file, exe_name, opts.dev_mode)?;

    // 4. Clean up object file
    if !opts.keep_obj {
        let _ = fs::remove_file(&obj_file);
    }

    Ok(())
}

/// Links the generated object file to a native executable using the system linker.
/// - On Windows: tries clang, then lld-link with auto-detected SDK paths, then MSVC link.exe
/// - On macOS/Linux: uses clang as the linker (adds C runtime, startup code)
/// Returns Ok(()) on success, or an error string on failure.
fn link_object_file(obj_file: &str, output: &str, dev_mode: bool) -> Result<(), String> {
    // Always use embedded linker for all platforms
    let embedded_linker = extract_embedded_linker()?;

    if cfg!(target_os = "windows") {
        let sdk_paths = find_windows_sdk_paths();

        let mut cmd = Command::new(&embedded_linker);
        cmd.arg(format!("/OUT:{}", output))
            .arg(obj_file)
            .arg("/SUBSYSTEM:CONSOLE")
            .arg("/ENTRY:main");

        if let Some(paths) = sdk_paths {
            if let Some(ucrt) = paths.ucrt_lib {
                cmd.arg(format!("/LIBPATH:{}", ucrt));
            }
            if let Some(um) = paths.um_lib {
                cmd.arg(format!("/LIBPATH:{}", um));
            }
            if let Some(msvc) = paths.msvc_lib {
                cmd.arg(format!("/LIBPATH:{}", msvc));
            }
            cmd.arg("ucrt.lib")
                .arg("vcruntime.lib")
                .arg("legacy_stdio_definitions.lib");
        }

        let output_result = cmd.output();
        match output_result {
            Ok(result) => {
                if dev_mode {}

                if result.status.success() {
                    Ok(())
                } else {
                    Err(format!(
                        "Linking failed. On Windows, you need Visual Studio Build Tools or Windows SDK.\n\
                        Download from: https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022"
                    ))
                }
            }
            Err(e) => Err(format!("Failed to run linker: {}", e)),
        }
    } else {
        // macOS and Linux
        let status = Command::new(&embedded_linker)
            .arg(obj_file)
            .arg("-o")
            .arg(output)
            .status();

        match status {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => Err(format!("Linking failed with status: {:?}", s.code())),
            Err(e) => Err(format!("Failed to run linker: {}", e)),
        }
    }
}

/// Windows SDK path information
struct WindowsSdkPaths {
    ucrt_lib: Option<String>,
    um_lib: Option<String>,
    msvc_lib: Option<String>,
}

/// Attempts to auto-detect Windows SDK and MSVC library paths
fn find_windows_sdk_paths() -> Option<WindowsSdkPaths> {
    // Try to find paths using common environment variables and locations
    let program_files_x86 = env::var("ProgramFiles(x86)").ok()?;

    // Try to find Windows Kits path
    let kits_base = format!("{}\\Windows Kits\\10\\Lib", program_files_x86);
    let kits_path = Path::new(&kits_base);

    let ucrt_lib = if kits_path.exists() {
        // Find the latest SDK version
        if let Ok(entries) = fs::read_dir(kits_path) {
            let mut versions: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            versions.sort();
            versions
                .last()
                .map(|v| format!("{}\\{}\\ucrt\\x64", kits_base, v))
        } else {
            None
        }
    } else {
        None
    };

    let um_lib = ucrt_lib.as_ref().map(|u| u.replace("ucrt", "um"));

    // Try to find MSVC path
    let msvc_base = format!("{}\\Microsoft Visual Studio", program_files_x86);
    let msvc_path = Path::new(&msvc_base);

    let msvc_lib = if msvc_path.exists() {
        // Look for Build Tools or any VS version
        find_msvc_lib_path(&msvc_base)
    } else {
        None
    };

    Some(WindowsSdkPaths {
        ucrt_lib,
        um_lib,
        msvc_lib,
    })
}

/// Helper to recursively find MSVC lib path
fn find_msvc_lib_path(base: &str) -> Option<String> {
    let base_path = Path::new(base);

    // Try common paths
    for year in &["2022", "2019", "2017"] {
        for edition in &["BuildTools", "Community", "Professional", "Enterprise"] {
            let vc_path = base_path.join(year).join(edition).join("VC\\Tools\\MSVC");
            if let Ok(entries) = fs::read_dir(&vc_path) {
                if let Some(version) = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .max()
                {
                    return Some(format!("{}\\{}\\lib\\x64", vc_path.display(), version));
                }
            }
        }
    }
    None
}

/// Helper to skip to the next statement after a parse error.
/// Advances the parser until the next semicolon or end of file.
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
