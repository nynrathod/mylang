use clap::{Parser, Subcommand};
use mylang::compiler::{compile_project, CompileOptions};
use std::fs;
use std::path::PathBuf;
use std::process::{exit, Command};

/// ===============================================================
/// wow CLI - User-facing command-line interface for the mylang language
///
/// This CLI provides a simple interface for building, running, and
/// checking mylang language projects. It is designed to be easy for
/// end users, hiding intermediate files unless requested.
///
/// Commands:
///   - `wow build`: Compiles the project to a native binary.
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
    Build {
        /// Path to the project directory or main.my file.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Name of the output binary.
        #[arg(short, long, default_value = "output")]
        output: String,
        /// Enable release optimizations.
        #[arg(short, long)]
        release: bool,
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
        Commands::Build {
            path,
            output,
            release,
            keep_ll,
        } => {
            // Set up compilation options for building.
            let opts = CompileOptions {
                input_path: path,
                output_name: output,
                dev_mode: false,
                keep_ll,
                release_mode: release,
                ..Default::default()
            };

            // Compile the project and print result.
            match compile_project(opts) {
                Ok(_) => println!("✓ Build successful"),
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
            // Use a temporary output name for the binary.
            let temp = ".output";
            let opts = CompileOptions {
                input_path: path,
                output_name: temp.to_string(),
                dev_mode: false,
                keep_ll,
                ..Default::default()
            };

            // Compile the project.
            if let Err(e) = compile_project(opts) {
                eprintln!("{}", e);
                exit(1);
            }

            // Determine the executable name based on platform.
            let exe = if cfg!(windows) {
                format!("{}.exe", temp)
            } else {
                format!("./{}", temp)
            };

            // Run the compiled binary, passing any additional arguments.
            let status = Command::new(&exe).args(&args).status();

            // Clean up the binary after running (script-like experience).
            let _ = fs::remove_file(&exe);

            // Handle the exit status of the program.
            match status {
                Ok(s) if s.success() => {}
                Ok(s) => exit(s.code().unwrap_or(1)),
                Err(e) => {
                    eprintln!("Failed to run: {}", e);
                    exit(1);
                }
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
                Ok(_) => println!("✓ No errors found"),
                Err(e) => {
                    eprintln!("{}", e);
                    exit(1);
                }
            }
        }
    }
}
