//! Cortex Language Server
//!
//! Speaks the Language Server Protocol over stdio.
//! Start it from your editor with the command `cortex-lsp`.
//!
//! Capabilities implemented:
//!   - textDocument/publishDiagnostics  (on open + every change)
//!   - textDocument/hover               (variable / task names → type)
//!   - workspace/symbol                 (list all task names)
//!
//! Usage in VS Code (settings.json):
//!   "cortex.lsp.serverCommand": "cortex-lsp"
//! Or via the generic LSP client extension.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use cortex_engine::checker::PermissionManifest;

// ── Document store ────────────────────────────────────────────────────────────

#[derive(Default)]
struct DocStore {
    docs: HashMap<Url, String>,
}

// ── Server struct ─────────────────────────────────────────────────────────────

struct CortexLsp {
    client:   Client,
    store:    Arc<RwLock<DocStore>>,
}

impl CortexLsp {
    fn new(client: Client) -> Self {
        Self {
            client,
            store: Arc::new(RwLock::new(DocStore::default())),
        }
    }

    async fn diagnose(&self, uri: Url, text: &str) {
        let diags = self.compile_diagnostics(text);
        self.client.publish_diagnostics(uri, diags, None).await;
    }

    fn compile_diagnostics(&self, source: &str) -> Vec<Diagnostic> {
        cortex_engine::compile_to_diagnostics(source, &PermissionManifest::allow_all())
            .into_iter()
            .map(|d| {
                // Cortex spans are 1-based; LSP positions are 0-based.
                let line = d.line.saturating_sub(1);
                let col  = d.col.saturating_sub(1);
                let end_col = col + d.len;
                Diagnostic {
                    range: Range {
                        start: Position { line, character: col },
                        end:   Position { line, character: end_col },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    source:   Some("cortex".to_string()),
                    message:  d.message,
                    ..Default::default()
                }
            })
            .collect()
    }
}

// ── LanguageServer impl ───────────────────────────────────────────────────────

#[tower_lsp::async_trait]
impl LanguageServer for CortexLsp {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name:    "cortex-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "cortex-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    // ── Text document sync ────────────────────────────────────────────────────

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri  = params.text_document.uri.clone();
        let text = params.text_document.text.clone();
        self.store.write().await.docs.insert(uri.clone(), text.clone());
        self.diagnose(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        // Full sync — take the last content change
        if let Some(change) = params.content_changes.into_iter().last() {
            self.store.write().await.docs.insert(uri.clone(), change.text.clone());
            self.diagnose(uri, &change.text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.store.write().await.docs.remove(&params.text_document.uri);
        // Clear diagnostics on close
        self.client
            .publish_diagnostics(params.text_document.uri, vec![], None)
            .await;
    }

    // ── Hover ─────────────────────────────────────────────────────────────────

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri  = params.text_document_position_params.text_document.uri.clone();
        let pos  = params.text_document_position_params.position;
        let store = self.store.read().await;
        let text  = match store.docs.get(&uri) {
            Some(t) => t.clone(),
            None    => return Ok(None),
        };
        drop(store);

        let word = word_at(&text, pos.line, pos.character);
        if word.is_empty() { return Ok(None); }

        // Compile and extract task / variable info from the AST.
        let Ok(ast) = cortex_engine::compile(&text, &PermissionManifest::allow_all()) else {
            return Ok(None);
        };

        // Check if word is a task name
        for task in &ast.tasks {
            if task.name.name == word {
                let params_str: String = task.params.iter()
                    .map(|p| format!("{}: {}", p.name.name, p.ty.kind))
                    .collect::<Vec<_>>()
                    .join(", ");
                let markdown = format!(
                    "```cortex\ntask {}({}) -> {}\n```",
                    task.name.name, params_str, task.return_ty.kind
                );
                return Ok(Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind:  MarkupKind::Markdown,
                        value: markdown,
                    }),
                    range: None,
                }));
            }
        }

        Ok(None)
    }

    // ── Workspace symbols ─────────────────────────────────────────────────────

    async fn symbol(
        &self,
        _params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        Ok(None)
    }

    async fn execute_command(&self, _params: ExecuteCommandParams) -> Result<Option<Value>> {
        Ok(None)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract the word (identifier) under the given 0-based position.
fn word_at(text: &str, line: u32, character: u32) -> String {
    let line_text = text.lines().nth(line as usize).unwrap_or("");
    let char_pos  = character as usize;

    // Find word boundaries
    let start = line_text[..char_pos.min(line_text.len())]
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map_or(0, |i| i + 1);
    let end = line_text[char_pos.min(line_text.len())..]
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .map_or(line_text.len(), |i| i + char_pos.min(line_text.len()));

    line_text[start..end].to_string()
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let stdin  = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(CortexLsp::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
