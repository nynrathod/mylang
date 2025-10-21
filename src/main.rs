use mylang::compiler::{compile_project, CompileOptions};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let opts = CompileOptions {
        input_path: PathBuf::from("./examples/myproject/main.my"),
        // input_path: PathBuf::from("./test_cases.md"),
        output_name: "output".to_string(),
        dev_mode: true,
        print_ast: true,
        print_mir: true,
        keep_ll: true,
        check_only: false,
        release_mode: false,
    };

    let output_name = opts.output_name.clone();
    match compile_project(opts) {
        Ok(result) => {
            if result.success {
                println!("\n✓ Compilation successful");

                // Run the generated binary (dev mode)
                let exe = if cfg!(windows) {
                    format!("{}.exe", output_name)
                } else {
                    format!("./{}", output_name)
                };

                // Pass no args for dev mode, but you can customize if needed
                match Command::new(&exe).status() {
                    Ok(status) => {
                        if !status.success() {
                            eprintln!("Program exited with status: {}", status);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to run binary '{}': {}", exe, e);
                    }
                }

                // Optionally clean up the binary after running in dev mode
                if let Err(e) = fs::remove_file(&exe) {
                    eprintln!("Warning: failed to remove binary '{}': {}", exe, e);
                }
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
