use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cortex", about = "The Cortex language compiler and runtime")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile a Cortex script
    Compile {
        /// Source file (.ctx)
        file: PathBuf,

        /// Output format
        #[arg(long, value_enum, default_value = "ast")]
        emit: EmitFormat,

        /// Comma-separated list of allowed native APIs (e.g. native.log,native.fs.read)
        #[arg(long, value_delimiter = ',')]
        allow: Vec<String>,

        /// Output file for binary bytecode (--emit=bytecode only)
        #[arg(short = 'o')]
        output: Option<PathBuf>,
    },

    /// Compile and execute a Cortex script
    Run {
        /// Source file (.ctx)
        file: PathBuf,

        /// Comma-separated list of allowed native APIs
        #[arg(long, value_delimiter = ',')]
        allow: Vec<String>,

        /// Maximum VM op budget (default: 10000)
        #[arg(long, default_value = "10000")]
        ops_limit: u64,
    },
}

#[derive(Clone, ValueEnum)]
enum EmitFormat {
    /// JSON Abstract Syntax Tree (default)
    Ast,
    /// Human-readable text bytecode assembly
    Asm,
    /// Binary bytecode (.ctxb) — Phase 3
    Bytecode,
    /// WebAssembly Text Format (.wat)
    Wat,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Compile { file, emit, allow, output } => {
            let source = read_file(&file);
            let manifest = cortex_engine::checker::PermissionManifest::new(allow);

            match cortex_engine::compile(&source, &manifest) {
                Ok(ast) => match emit {
                    EmitFormat::Ast => {
                        println!("{}", serde_json::to_string_pretty(&ast).unwrap());
                    }
                    EmitFormat::Asm => {
                        let chunks = cortex_engine::compile_to_chunks(&ast);
                        for chunk in &chunks {
                            print!("{}", cortex_engine::compiler::disassemble(chunk));
                        }
                    }
                    EmitFormat::Bytecode => {
                        eprintln!("error: --emit=bytecode binary format is available in Phase 3");
                        std::process::exit(1);
                    }
                    EmitFormat::Wat => {
                        let wat = cortex_engine::compile_to_wat(&ast);
                        match output {
                            None => print!("{}", wat),
                            Some(path) => {
                                if let Err(e) = std::fs::write(&path, &wat) {
                                    eprintln!("error: could not write '{}': {}", path.display(), e);
                                    std::process::exit(1);
                                }
                            }
                        }
                    }
                },
                Err(errors) => {
                    for e in &errors { eprintln!("{}", e); }
                    std::process::exit(1);
                }
            }
        }

        Command::Run { file, allow, ops_limit } => {
            let source = read_file(&file);
            let manifest = cortex_engine::checker::PermissionManifest::new(allow);

            let ast = match cortex_engine::compile(&source, &manifest) {
                Ok(a) => a,
                Err(errors) => {
                    for e in &errors { eprintln!("{}", e); }
                    std::process::exit(1);
                }
            };

            let chunks = cortex_engine::compile_to_chunks(&ast);
            let Some(chunk) = chunks.into_iter().next() else {
                eprintln!("error: no tasks found in '{}'", file.display());
                std::process::exit(1);
            };

            let mut registry = cortex_engine::vm::CliRegistry;
            match cortex_engine::vm::Vm::new(chunk, ops_limit, &mut registry).run() {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn read_file(path: &PathBuf) -> String {
    match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: could not read '{}': {}", path.display(), e);
            std::process::exit(1);
        }
    }
}
