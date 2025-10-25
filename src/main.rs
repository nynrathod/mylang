use clap::Parser;
use mylang::compiler::{compile_project, CompileOptions};
use std::path::PathBuf;
use std::process::Command;

/// mylang compiler CLI (single binary, supports embedded linker)
#[derive(Parser)]
#[command(name = "mylang")]
#[command(about = "mylang compiler")]
#[command(version)]
struct Args {
    /// Path to the project directory or main.my file
    #[clap(default_value = ".")]
    input_path: String,

    /// Keep the generated LLVM IR (.ll) file for debugging
    #[clap(long)]
    keep_ll: bool,

    /// Only check for errors, do not build
    #[clap(long)]
    check_only: bool,

    /// Run immediately after compilation (for dev/testing)
    #[clap(long)]
    run: bool,
}

fn main() {
    let args = Args::parse();

    let input_path = PathBuf::from(&args.input_path);

    // Environment variable overrides (for wow CLI integration)
    let output_name = std::env::var("MYLANG_OUTPUT_NAME").unwrap_or_else(|_| "output".to_string());
    let check_only = std::env::var("MYLANG_CHECK_ONLY").is_ok() || args.check_only;

    // In dev mode (cargo run), always keep .ll; in prod, only if --keep-ll is passed
    let keep_ll = if cfg!(debug_assertions) {
        true
    } else {
        args.keep_ll
    };

    let opts = CompileOptions {
        input_path,
        output_name: output_name.clone(),
        dev_mode: cfg!(debug_assertions),
        print_ast: cfg!(debug_assertions),
        print_mir: cfg!(debug_assertions),
        keep_ll,
        keep_obj: false,
        check_only,
    };

    match compile_project(opts) {
        Ok(result) => {
            if result.success {
                if check_only {
                    println!("✓ No errors found");
                }

                // In dev mode (cargo run), if not --run, run the output binary after compilation and print its output, then delete the binary.
                if (cfg!(debug_assertions) && !args.check_only && !args.run)
                    || (args.run && !check_only)
                {
                    if let Some(exe_path) = result.exe_path {
                        println!("\nRunning executable...\n");
                        println!("{}", "=".repeat(50));

                        // Capture and print output
                        match Command::new(&exe_path).output() {
                            Ok(output) => {
                                print!("{}", String::from_utf8_lossy(&output.stdout));
                                eprint!("{}", String::from_utf8_lossy(&output.stderr));
                                println!("{}", "=".repeat(50));
                                if !output.status.success() {
                                    eprintln!("\nProgram exited with status: {}", output.status);
                                } else {
                                    println!("\n✓ Program executed successfully");
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to run binary: {}", e);
                            }
                        }

                        // Clean up binary after run
                        let _ = std::fs::remove_file(&exe_path);
                    }
                }
            } else {
                eprintln!("✗ Compilation failed with {} error(s)", result.error_count);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
    // Only clean up the output binary if not running under wow (WOW_RUNNING not set)
    if std::env::var("WOW_RUNNING").is_err() {
        let output_name =
            std::env::var("MYLANG_OUTPUT_NAME").unwrap_or_else(|_| "output".to_string());
        let exe_name = if cfg!(windows) {
            format!("{}.exe", output_name)
        } else {
            output_name
        };
        let exe_path = std::env::current_dir().unwrap().join(&exe_name);
        let _ = std::fs::remove_file(&exe_path);
    }
}
