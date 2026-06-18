use clap::{Parser, Subcommand, ValueEnum};
use std::collections::HashSet;
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

        /// Path to a cortex.manifest.json permission manifest file
        #[arg(long)]
        manifest: Option<PathBuf>,

        /// Output file path (used with --emit=bytecode, --emit=wat, --emit=wasm)
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

        /// Path to a cortex.manifest.json permission manifest file
        #[arg(long)]
        manifest: Option<PathBuf>,

        /// Maximum VM op budget (default: 10000)
        #[arg(long, default_value = "10000")]
        ops_limit: u64,
    },
}

// ── Manifest JSON format (mirrors cortex.manifest.json) ──────────────────────

#[derive(serde::Deserialize)]
struct ManifestFile {
    apps: std::collections::HashMap<String, ManifestApp>,
}

#[derive(serde::Deserialize)]
struct ManifestApp {
    #[serde(default)]
    allow: Vec<String>,
}

/// Load a manifest JSON file and return the union of all allowed APIs across
/// all origins. An empty `allow` list for any origin means "allow everything"
/// for that origin, which causes the whole manifest to be treated as allow-all.
fn load_manifest_file(path: &PathBuf) -> Result<Vec<String>, String> {
    let data = std::fs::read_to_string(path)
        .map_err(|e| format!("error: could not read manifest '{}': {}", path.display(), e))?;
    let mf: ManifestFile = serde_json::from_str(&data)
        .map_err(|e| format!("error: invalid manifest '{}': {}", path.display(), e))?;

    let mut apis: HashSet<String> = HashSet::new();
    for app in mf.apps.values() {
        if app.allow.is_empty() {
            // Empty allow list = allow all → return empty vec (allow-all sentinel)
            return Ok(vec![]);
        }
        for api in &app.allow {
            apis.insert(api.clone());
        }
    }
    Ok(apis.into_iter().collect())
}

/// Build a PermissionManifest from `--allow` flags and an optional `--manifest` file.
/// APIs from both sources are unioned together.
fn build_permission_manifest(
    allow: Vec<String>,
    manifest_path: Option<&PathBuf>,
) -> cortex_engine::checker::PermissionManifest {
    let mut combined = allow;
    if let Some(path) = manifest_path {
        match load_manifest_file(path) {
            Ok(apis) => {
                if apis.is_empty() {
                    // allow-all from manifest
                    return cortex_engine::checker::PermissionManifest::allow_all();
                }
                combined.extend(apis);
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }
    cortex_engine::checker::PermissionManifest::new(combined)
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
    /// Binary WebAssembly module (.wasm)
    Wasm,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Compile { file, emit, allow, manifest: manifest_path, output } => {
            let source = read_file(&file);
            let manifest = build_permission_manifest(allow, manifest_path.as_ref());

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
                        let chunks = cortex_engine::compile_to_chunks(&ast);
                        let bytes = cortex_engine::encode_chunks(&chunks);
                        let path = output.unwrap_or_else(|| {
                            let mut p = file.clone();
                            p.set_extension("ctxb");
                            p
                        });
                        if let Err(e) = std::fs::write(&path, &bytes) {
                            eprintln!("error: could not write '{}': {}", path.display(), e);
                            std::process::exit(1);
                        }
                        eprintln!("wrote {} bytes to '{}'", bytes.len(), path.display());
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
                    EmitFormat::Wasm => {
                        match cortex_engine::compile_to_wasm(&ast) {
                            Ok(bytes) => {
                                let path = output.unwrap_or_else(|| {
                                    let mut p = file.clone();
                                    p.set_extension("wasm");
                                    p
                                });
                                if let Err(e) = std::fs::write(&path, &bytes) {
                                    eprintln!("error: could not write '{}': {}", path.display(), e);
                                    std::process::exit(1);
                                }
                                eprintln!("wrote {} bytes to '{}'", bytes.len(), path.display());
                            }
                            Err(e) => {
                                eprintln!("{e}");
                                std::process::exit(1);
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

        Command::Run { file, allow, manifest: manifest_path, ops_limit } => {
            let source = read_file(&file);
            let manifest = build_permission_manifest(allow, manifest_path.as_ref());

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
