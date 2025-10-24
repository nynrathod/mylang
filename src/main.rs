use clap::Parser;
use mylang::compiler::{compile_project, CompileOptions};
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
struct Args {
    /// Path to the project directory or main.my file
    #[clap(default_value = ".")]
    input_path: String,
    /// Keep the generated LLVM IR (.ll) file for debugging
    #[clap(long)]
    keep_ll: bool,
}

fn main() {
    let args = Args::parse();

    // Debug print for input_path and canonicalized path
    let input_path = PathBuf::from(&args.input_path);
    println!("DEBUG: input_path = {:?}", input_path);
    match std::fs::canonicalize(&input_path) {
        Ok(canon) => println!("DEBUG: canonicalized input_path = {:?}", canon),
        Err(e) => println!("DEBUG: canonicalize error: {:?}", e),
    }

    let keep_ll = if cfg!(debug_assertions) {
        true
    } else {
        args.keep_ll
    };

    let opts = CompileOptions {
        input_path,
        output_name: "output".to_string(),
        dev_mode: true,
        print_ast: true,
        print_mir: true,
        keep_ll,
        keep_obj: false,
        check_only: false,
    };

    match compile_project(opts) {
        Ok(result) => {
            if result.success {
                println!("\n✓ Compilation successful");

                // Run the generated binary (dev mode)
                if let Some(exe_path) = result.exe_path {
                    println!("\nRunning executable...\n");
                    println!("{}", "=".repeat(50));

                    match Command::new(&exe_path).status() {
                        Ok(status) => {
                            println!("{}", "=".repeat(50));
                            if !status.success() {
                                eprintln!("\nProgram exited with status: {}", status);
                            } else {
                                println!("\n✓ Program executed successfully");
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to run binary '{}': {}", exe_path.display(), e);
                        }
                    }

                    // Optionally clean up the binary after running in dev mode
                    if let Err(e) = std::fs::remove_file(&exe_path) {
                        eprintln!(
                            "Warning: failed to remove binary '{}': {}",
                            exe_path.display(),
                            e
                        );
                    } else {
                        println!("Cleaned up binary: {}", exe_path.display());
                    }
                }
            } else {
                eprintln!(
                    "\n✗ Compilation failed with {} error(s)",
                    result.error_count
                );
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
