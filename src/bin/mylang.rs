use clap::{Parser, Subcommand};
use mylang::compiler::{compile_project, CompileOptions};
use std::fs;
use std::path::PathBuf;
use std::process::{exit, Command};

#[derive(Parser)]
#[command(name = "mylang")]
#[command(about = "Rust-like language with simple syntax")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Build {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(short, long, default_value = "output")]
        output: String,
        #[arg(short, long)]
        release: bool,
        #[arg(long)]
        keep_ll: bool,
    },
    Run {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        keep_ll: bool,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    Check {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build {
            path,
            output,
            release,
            keep_ll,
        } => {
            let opts = CompileOptions {
                input_path: path,
                output_name: output,
                dev_mode: false,
                keep_ll,
                release_mode: release,
                ..Default::default()
            };

            match compile_project(opts) {
                Ok(_) => println!("✓ Build successful"),
                Err(e) => {
                    eprintln!("{}", e);
                    exit(1);
                }
            }
        }

        Commands::Run {
            path,
            keep_ll,
            args,
        } => {
            let temp = ".output";
            let opts = CompileOptions {
                input_path: path,
                output_name: temp.to_string(),
                dev_mode: false,
                keep_ll,
                ..Default::default()
            };

            if let Err(e) = compile_project(opts) {
                eprintln!("{}", e);
                exit(1);
            }

            let exe = if cfg!(windows) {
                format!("{}.exe", temp)
            } else {
                format!("./{}", temp)
            };
            let status = Command::new(&exe).args(&args).status();
            let _ = fs::remove_file(&exe);

            match status {
                Ok(s) if s.success() => {}
                Ok(s) => exit(s.code().unwrap_or(1)),
                Err(e) => {
                    eprintln!("Failed to run: {}", e);
                    exit(1);
                }
            }
        }

        Commands::Check { path } => {
            let opts = CompileOptions {
                input_path: path,
                check_only: true,
                dev_mode: false,
                ..Default::default()
            };

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
