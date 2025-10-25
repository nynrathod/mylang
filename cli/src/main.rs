use clap::{Parser, Subcommand};
use std::env;
use std::path::PathBuf;
use std::process::{exit, Command};

#[derive(Parser)]
#[command(name = "wow")]
#[command(about = "mylang language CLI")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the project to a persistent binary
    Build {
        /// Name of the output binary
        #[arg(short, long, default_value = "output")]
        output: String,

        /// Keep the generated LLVM IR (.ll) file
        #[arg(long)]
        keep_ll: bool,
    },

    /// Compile and run immediately (auto-cleanup)
    Run {
        /// Path to the project directory or main.my file
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Keep the generated LLVM IR (.ll) file
        #[arg(long)]
        keep_ll: bool,

        /// Arguments to pass to the program
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Check for errors without compiling
    Check {
        /// Path to the project directory or main.my file
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

fn main() {
    if std::env::args().len() == 1 {
        println!("🎉 wow CLI - mylang language tool");
        println!("Type `wow --help` for usage");
        return;
    }

    let cli = Cli::parse();

    match cli.command {
        Commands::Build { output, keep_ll } => {
            let mylang_exe = find_mylang_exe();

            let mut cmd = Command::new(&mylang_exe);
            cmd.arg(".")
                .env("MYLANG_OUTPUT_NAME", output)
                .env("WOW_RUNNING", "1");

            if keep_ll {
                cmd.arg("--keep-ll");
            }

            match cmd.status() {
                Ok(s) if s.success() => {}
                Ok(s) => exit(s.code().unwrap_or(1)),
                Err(e) => {
                    eprintln!("Failed to run mylang: {}", e);
                    exit(1);
                }
            }
        }

        Commands::Run {
            path,
            keep_ll,
            args,
        } => {
            let mylang_exe = find_mylang_exe();

            // Generate unique temp binary name
            let temp_name = format!("temp_mylang_{}", std::process::id());
            let temp_obj_name = format!("{}.o", temp_name);

            // Compile to temp binary, pass temp object name as env var
            let mut cmd = Command::new(&mylang_exe);
            cmd.arg(path.to_string_lossy().to_string())
                .env("MYLANG_OUTPUT_NAME", &temp_name)
                .env("MYLANG_OBJ_NAME", &temp_obj_name)
                .env("WOW_RUNNING", "1");

            if keep_ll {
                cmd.arg("--keep-ll");
            }

            // Compile
            match cmd.status() {
                Ok(s) if !s.success() => exit(s.code().unwrap_or(1)),
                Err(e) => {
                    eprintln!("Failed to compile: {}", e);
                    exit(1);
                }
                _ => {}
            }

            // Run the temp binary
            let exe_name = if cfg!(windows) {
                format!("{}.exe", temp_name)
            } else {
                temp_name.clone()
            };
            let exe_path = std::env::current_dir().unwrap().join(&exe_name);

            // Run the temp binary and capture output/errors
            let output = Command::new(&exe_path).args(&args).output();

            match output {
                Ok(out) => {
                    print!("{}", String::from_utf8_lossy(&out.stdout));
                    eprint!("{}", String::from_utf8_lossy(&out.stderr));
                    if !out.status.success() {
                        let _ = std::fs::remove_file(&exe_path);
                        exit(out.status.code().unwrap_or(1));
                    }
                }
                Err(e) => {
                    eprintln!("Failed to start process: {}", e);
                    let _ = std::fs::remove_file(&exe_path);
                    exit(1);
                }
            }
            // Always attempt to delete the temp binary after running, regardless of success/failure
            let _ = std::fs::remove_file(&exe_path);
        }

        Commands::Check { path } => {
            let mylang_exe = find_mylang_exe();

            let mut cmd = Command::new(&mylang_exe);
            cmd.arg(path.to_string_lossy().to_string())
                .env("MYLANG_CHECK_ONLY", "1");

            match cmd.status() {
                Ok(s) if s.success() => {}
                Ok(s) => exit(s.code().unwrap_or(1)),
                Err(e) => {
                    eprintln!("Failed to check: {}", e);
                    exit(1);
                }
            }
        }
    }
}

/// Find mylang executable - check same directory first, then PATH
fn find_mylang_exe() -> String {
    let mylang_name = if cfg!(windows) {
        "mylang.exe"
    } else {
        "mylang"
    };

    // Try same directory as wow
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let local_mylang = exe_dir.join(mylang_name);
            if local_mylang.exists() {
                return local_mylang.to_string_lossy().to_string();
            }
        }
    }

    // Try PATH
    if Command::new(mylang_name).arg("--version").output().is_ok() {
        return mylang_name.to_string();
    }

    eprintln!("Error: 'mylang' executable not found!");
    eprintln!("Make sure 'mylang' is in the same directory as 'wow' or in your PATH");
    exit(1);
}
