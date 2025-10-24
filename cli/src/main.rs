use clap::{Parser, Subcommand};
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
    // Show welcome message if no arguments are provided
    if std::env::args().len() == 1 {
        println!("🎉 wow CLI installed! Type `wow --help` for usage.");
        return;
    }

    let cli = Cli::parse();

    match cli.command {
        // =========================
        // Build Command
        // =========================
        Commands::Build {
            output: _,
            keep_ll: _,
        } => {
            println!(
                "wow build is not supported in pure CLI mode. Use mylang.exe directly for builds."
            );
            exit(1);
        }

        // =========================
        // Run Command
        // =========================
        Commands::Run {
            path,
            keep_ll,
            args,
        } => {
            // Try to find mylang.exe in PATH or current directory
            let mylang_exe = if cfg!(windows) {
                "mylang.exe"
            } else {
                "mylang"
            };
            let mut mylang_args = vec![path.to_string_lossy().to_string()];
            if keep_ll {
                mylang_args.push("--keep-ll".to_string());
            }
            mylang_args.extend(args);

            let status = Command::new(mylang_exe).args(&mylang_args).status();

            match status {
                Ok(s) if s.success() => {}
                Ok(s) => exit(s.code().unwrap_or(1)),
                Err(e) => {
                    eprintln!("Failed to run mylang.exe: {}", e);
                    exit(1);
                }
            }
        }

        // =========================
        // Check Command
        // =========================
        Commands::Check { path: _ } => {
            println!(
                "wow check is not supported in pure CLI mode. Use mylang.exe directly for checks."
            );
            exit(1);
        }
    }
}
