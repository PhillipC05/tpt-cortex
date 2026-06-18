# TPT Cortex

Give PWAs native superpowers — without the app store.

TPT Cortex is a custom statically-typed DSL and lightweight VM that lets web apps declare background tasks, hardware interactions, and local data operations executed by a native companion process. No WebView jail. No Play Store. No iOS App Store.

## Architecture

```
PWA (Svelte)  <──WebSocket──>  cortex-daemon (Go)  <──FFI──>  cortex-engine (Rust)
                127.0.0.1:9911                                  Lexer → Parser → VM
```

## Components

| Component | Language | Purpose |
|---|---|---|
| `cortex-engine` | Rust | Compiler (lexer, parser, type checker, bytecode) + VM + CLI |
| `cortex-daemon` | Go | Native IPC host, WebSocket server, background scheduler |
| `cortex-shell` | Rust | Desktop WebView host (Tauri WRY) — Phase 3 |
| `cortex-pwa` | Svelte + TS | Demo PWA frontend — Phase 3 |
| `cortex-android` | Kotlin | Android companion app (TPT Core) — Phase 4 |

## Quick Start

```bash
# Phase 1 — compile a Cortex script
cargo run -p cortex-engine -- compile examples/hello.ctx --allow=native.log

# Phase 2 — run a Cortex script
cargo run -p cortex-engine -- run examples/hello.ctx --allow=native.log
```

## The Cortex Language

```cortex
task syncNotes(userId: string) -> void {
    let notes: list<string> = native.db.query("SELECT * FROM notes WHERE synced = 0");

    if native.net.isConnected() {
        let payload: string = native.json.encode(notes);
        native.http.post("https://api.example.com/sync", payload);
    }

    native.log("Done");
}
```

## Roadmap

- **Phase 1 (Weeks 1–3):** Core compiler — lexer, parser, type checker, CLI
- **Phase 2 (Weeks 4–6):** VM — bytecode compiler, stack-based execution, op budgeting
- **Phase 3 (Weeks 7–9):** Native bridging — Go daemon, IPC, Svelte PWA
- **Phase 4 (Weeks 10–12):** Pulse daemon, background scheduler, Android companion

## Distribution

TPT Core (Android) is distributed via direct APK download — no app store required.
