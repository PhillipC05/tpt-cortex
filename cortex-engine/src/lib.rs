pub mod ast;
pub mod checker;
pub mod compiler;
pub mod ffi;
pub mod lexer;
pub mod parser;
pub mod vm;
pub mod wasm;

use ast::Program;
use checker::PermissionManifest;

/// Phase 1: Lex → Parse → Type-check. Returns the validated AST or error strings.
pub fn compile(source: &str, manifest: &PermissionManifest) -> Result<Program, Vec<String>> {
    let tokens = lexer::tokenize(source).map_err(|e| vec![e.to_string()])?;
    let ast = parser::parse(tokens).map_err(|errs| errs.iter().map(|e| e.to_string()).collect::<Vec<_>>())?;
    checker::check(&ast, manifest).map_err(|errs| errs.iter().map(|e| e.to_string()).collect::<Vec<_>>())?;
    Ok(ast)
}

/// Phase 2: Compile validated AST to bytecode chunks (one per task).
pub fn compile_to_chunks(ast: &Program) -> Vec<compiler::Chunk> {
    compiler::compile_program(ast)
}

/// Phase 3: Encode compiled chunks to binary `.ctxb` format.
pub fn encode_chunks(chunks: &[compiler::Chunk]) -> Vec<u8> {
    compiler::encode_chunks(chunks)
}

/// Phase 3: Decode a `.ctxb` binary blob back into chunks.
pub fn decode_chunks(data: &[u8]) -> Result<Vec<compiler::Chunk>, String> {
    compiler::decode_chunks(data)
}

/// Post-MVP: Compile validated AST to WebAssembly Text Format (.wat).
pub fn compile_to_wat(ast: &Program) -> String {
    wasm::compile_to_wat(ast)
}

/// Post-MVP: Compile validated AST to binary WebAssembly (.wasm).
pub fn compile_to_wasm(ast: &Program) -> Result<Vec<u8>, String> {
    wasm::compile_to_wasm(ast)
}

// ── LSP diagnostic API ────────────────────────────────────────────────────────

/// A structured error with source position, suitable for IDEs and LSP clients.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number.
    pub col:  u32,
    /// Length of the error range in characters.
    pub len:  u32,
}

/// Run the full compiler pipeline and return all diagnostics (errors) with
/// position info. Clears on clean source — returns empty Vec when valid.
pub fn compile_to_diagnostics(source: &str, manifest: &PermissionManifest) -> Vec<Diagnostic> {
    let tokens = match lexer::tokenize(source) {
        Ok(t) => t,
        Err(e) => {
            let span = lex_error_span(&e);
            return vec![Diagnostic {
                message: e.to_string(),
                line: span.map_or(1, |s| s.line),
                col:  span.map_or(1, |s| s.col),
                len:  span.map_or(1, |s| s.len),
            }];
        }
    };

    let ast = match parser::parse(tokens) {
        Ok(a) => a,
        Err(errs) => {
            return errs.iter().map(|e| {
                let span = parse_error_span(e);
                Diagnostic {
                    message: e.to_string(),
                    line: span.map_or(1, |s| s.line),
                    col:  span.map_or(1, |s| s.col),
                    len:  span.map_or(1, |s| s.len),
                }
            }).collect();
        }
    };

    match checker::check(&ast, manifest) {
        Ok(_) => vec![],
        Err(errs) => errs.iter().map(|e| {
            let span = check_error_span(e);
            Diagnostic {
                message: e.to_string(),
                line: span.line,
                col:  span.col,
                len:  span.len,
            }
        }).collect(),
    }
}

fn lex_error_span(e: &lexer::LexError) -> Option<lexer::Span> {
    use lexer::LexError::*;
    match e {
        UnexpectedChar   { span, .. } => Some(*span),
        UnterminatedString { span }   => Some(*span),
        InvalidEscape    { span, .. } => Some(*span),
        InvalidNumber    { span, .. } => Some(*span),
    }
}

fn parse_error_span(e: &parser::ParseError) -> Option<lexer::Span> {
    use parser::ParseError::*;
    match e {
        Unexpected    { span, .. } => Some(*span),
        UnexpectedEof { .. }       => None,
        Custom        { span, .. } => Some(*span),
    }
}

fn check_error_span(e: &checker::CheckError) -> lexer::Span {
    use checker::CheckError::*;
    match e {
        TypeMismatch    { span, .. } => *span,
        PermissionDenied{ span, .. } => *span,
        Undefined       { span, .. } => *span,
        AlreadyDeclared { span, .. } => *span,
        Custom          { span, .. } => *span,
    }
}
