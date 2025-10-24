use clap::{Parser, Subcommand};
use mylang::compiler::{compile_project, CompileOptions};
use std::fs;
use std::path::PathBuf;
use std::process::{exit, Command};

/// ===============================================================
/// wow CLI - User-facing command-line interface for the mylang language
///
/// This CLI provides a simple interface for building, running, and
/// checking mylang language projects. All commands work only in the
/// current directory where main.my is located.
///
/// Commands:
///   - `wow build`: Compiles the project to a native binary (always optimized).
///   - `wow run`: Compiles and immediately runs the project.
///   - `wow check`: Checks for errors without compiling to a binary.
///
/// ===============================================================

/// The main CLI struct.
/// Handles parsing of subcommands and arguments.
#[derive(Parser)]
#[command(name = "wow")]
#[command(about = "mylang cli")]
#[command(version)]
#[command(
    long_about = "===============================================================\n\
    wow CLI - User-facing command-line interface for the mylang language\n\
    \n\
    This CLI provides a simple interface for building, running, and\n\
    checking mylang language projects. All commands work only in the\n\
    current directory where main.my is located.\n\
    \n\
    Commands:\n\
      - `wow build`: Compiles the project to a native binary (always optimized).\n\
      - `wow run`: Compiles and immediately runs the project.\n\
      - `wow check`: Checks for errors without compiling to a binary.\n\
    \n\
    ===============================================================\n\
    The main CLI struct. Handles parsing of subcommands and arguments."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Enum for all supported subcommands.
/// Each variant corresponds to a CLI command.
#[derive(Subcommand)]
enum Commands {
    /// Build the project to a native binary.
    /// By default, outputs to `output` (or `output.exe` on Windows).
    /// Works only in the current directory.
    /// Always uses release optimizations for best performance.
    Build {
        /// Name of the output binary.
        #[arg(short, long, default_value = "output")]
        output: String,
        /// Keep the generated LLVM IR (.ll) file for debugging.
        #[arg(long)]
        keep_ll: bool,
    },
    /// Build and immediately run the project.
    /// Cleans up the binary after execution (like `go run`).
    Run {
        /// Path to the project directory or main.my file.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Keep the generated LLVM IR (.ll) file for debugging.
        #[arg(long)]
        keep_ll: bool,
        /// Arguments to pass to the program.
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Check the project for errors without compiling to a binary.
    Check {
        /// Path to the project directory or main.my file.
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

/// Entry point for the wow CLI.
/// Parses arguments and dispatches to the appropriate command handler.
fn main() {
    let cli = Cli::parse();

    match cli.command {
        // =========================
        // Build Command
        // =========================
        Commands::Build { output, keep_ll } => {
            // Set up compilation options for building.
            let opts = CompileOptions {
                input_path: PathBuf::from("."),
                output_name: output,
                dev_mode: false,
                keep_ll,
                ..Default::default()
            };

            // Compile the project and print result.
            match compile_project(opts) {
                Ok(result) => {
                    if result.error_count > 0 {
                        eprintln!("Build failed with {} errors", result.error_count);
                        exit(1);
                    } else {
                        println!("✓ Build successful");
                    }
                }
                Err(e) => {
                    eprintln!("{}", e);
                    exit(1);
                }
            }
        }

        // =========================
        // Run Command
        // =========================
        Commands::Run {
            path,
            keep_ll,
            args,
        } => {
            use std::env;
            use std::path::PathBuf;

            fn find_mylang_exe() -> Option<PathBuf> {
                // 1. Try same directory as wow.exe
                if let Ok(current_exe) = env::current_exe() {
                    if let Some(dir) = current_exe.parent() {
                        let candidate = dir.join(if cfg!(windows) {
                            "mylang.exe"
                        } else {
                            "mylang"
                        });
                        if candidate.exists() {
                            return Some(candidate);
                        }
                    }
                }
                // 2. Try PATH
                if let Ok(paths) = env::var("PATH") {
                    for path in env::split_paths(&paths) {
                        let candidate = path.join(if cfg!(windows) {
                            "mylang.exe"
                        } else {
                            "mylang"
                        });
                        if candidate.exists() {
                            return Some(candidate);
                        }
                    }
                }
                None
            }

            if let Some(mylang_exe) = find_mylang_exe() {
                // Build arguments for mylang.exe
                let mut mylang_args = vec![path.to_string_lossy().to_string()];
                // Pass --keep-ll if requested
                if keep_ll {
                    mylang_args.push("--keep-ll".to_string());
                }
                // Pass any additional args
                mylang_args.extend(args);

                // Call mylang.exe as a subprocess
                let status = Command::new(mylang_exe).args(&mylang_args).status();

                match status {
                    Ok(s) if s.success() => {}
                    Ok(s) => exit(s.code().unwrap_or(1)),
                    Err(e) => {
                        eprintln!("Failed to run mylang.exe: {}", e);
                        exit(1);
                    }
                }
            } else {
                eprintln!("PATH: {:?}", std::env::var("PATH"));
                eprintln!("Current exe: {:?}", std::env::current_exe());
                eprintln!("Error: Could not find mylang executable. Please ensure mylang.exe is in the same directory as wow.exe or in your PATH.");
                exit(1);
            }
        }

        // =========================
        // Check Command
        // =========================
        Commands::Check { path } => {
            // Set up options for check-only mode.
            let opts = CompileOptions {
                input_path: path,
                check_only: true,
                dev_mode: false,
                ..Default::default()
            };

            // Run the check and print result.
            match compile_project(opts) {
                Ok(result) => {
                    if result.error_count > 0 {
                        println!("Found {} errors", result.error_count);
                        exit(1);
                    } else {
                        println!("✓ No errors found");
                    }
                }
                Err(e) => {
                    eprintln!("{}", e);
                    exit(1);
                }
            }
        }
    }
}
