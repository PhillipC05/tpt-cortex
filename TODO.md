# TPT Cortex — Task Checklist

## Phase 1: Core Compiler (Weeks 1–3)
> **Milestone:** `cortex compile hello.ctx` outputs valid JSON AST; invalid code rejected with line/column errors.

### Monorepo Scaffold
- [x] Create root `Cargo.toml` as Rust workspace (members: `cortex-engine`, `cortex-shell`)
- [x] Create `cortex-engine/Cargo.toml` with deps: `clap`, `serde`, `serde_json`, `thiserror`
- [x] Create `go.work` + `cortex-daemon/go.mod` (stub only)
- [x] Add `.gitignore` (Rust, Go, Node, Android targets)
- [x] Add `README.md`

### Formal Grammar
- [x] Write `cortex-engine/GRAMMAR.ebnf` — canonical EBNF covering all tokens and productions

### Lexer (`cortex-engine/src/lexer/`)
- [x] Define `Token` enum (keywords, types, literals, operators, delimiters, `NATIVE_DOT`)
- [x] Define `Span { line, col, len }` struct
- [x] Implement `Lexer::new(source: &str)` and `Lexer::tokenize() -> Vec<Spanned<Token>>`
- [x] Handle `native.` as `Native` keyword token + `.` dot tokens consumed by parser
- [x] Return precise `LexError` with span on invalid input
- [x] Unit tests: keywords, string escapes, integer/float literals, `native.` prefix, error cases

### AST (`cortex-engine/src/ast/`)
- [x] Define all AST node types with `Span` fields
  - [x] `Program`, `Task`
  - [x] `LetStmt`, `IfStmt`, `ReturnStmt`, `ExprStmt`
  - [x] `BinaryExpr`, `UnaryExpr`, `CallExpr`, `NativeCallExpr`, `IndexExpr`
  - [x] `Literal`, `Ident`, `TypeAnnotation`
- [x] Derive `serde::Serialize` on all nodes for JSON output

### Parser (`cortex-engine/src/parser/`)
- [x] Implement recursive descent for declarations and statements
- [x] Implement Pratt (precedence climbing) for expressions
- [x] Collect multiple `ParseError { span, message }` before aborting
- [x] Unit tests: valid programs, operator precedence, error recovery

### Semantic Analyzer / Type Checker (`cortex-engine/src/checker/`)
- [x] Implement `SymbolTable` with lexical scoping (push/pop scope)
- [x] Type-check `let` declarations and assignments
- [x] Type-check function call arguments and return types
- [x] Validate `native.*` calls against `PermissionManifest`
- [x] Return `TypeError` and `PermissionError` variants with spans
- [x] Unit tests: type mismatches, undeclared variables, permission violations

### CLI (`cortex-engine/src/main.rs`)
- [x] `cortex compile <file>` — lex → parse → type-check → print JSON AST to stdout
- [x] `cortex compile <file> --emit=ast` — explicit flag (same as default)
- [x] `cortex compile <file> --allow=native.log,native.fs.read` — permission manifest flag
- [x] Error output format: `error[E001]: type mismatch — expected i32, found string (line 4, col 12)`
- [x] Exit code 0 on success, 1 on any error

### Phase 1 Verification
- [x] `cortex compile hello.ctx --allow=native.log` → valid JSON AST
- [x] `cortex compile bad_types.ctx` → type error with line number
- [x] `cortex compile sneaky.ctx` (calls `native.fs.write` without `--allow`) → permission error

---

## Phase 2: Virtual Machine (Weeks 4–6)
> **Milestone:** `cortex run math.ctx` executes pure math/logic and prints result.

### Bytecode Compiler (`cortex-engine/src/compiler/`)
- [x] Define `Instruction` enum: `PUSH_I32`, `PUSH_F64`, `PUSH_STR`, `PUSH_BOOL`, `LOAD`, `STORE`, `ADD`, `SUB`, `MUL`, `DIV`, `EQ`, `NEQ`, `LT`, `LTE`, `GT`, `GTE`, `AND`, `OR`, `NOT`, `CALL_NATIVE`, `JUMP`, `JUMP_IF_FALSE`, `RETURN`, `HALT`
- [x] Define `Chunk { instructions, string_table, native_table, local_names }`
- [x] Implement `compile_program(ast: &Program) -> Vec<Chunk>` with backpatching for jumps
- [x] Text bytecode emitter (`--emit=asm` → human-readable listing with comments)
- [x] Binary bytecode writer (`-o out.ctxb`) — `cortex compile <file> --emit=bytecode`

### VM (`cortex-engine/src/vm/`)
- [x] Implement value stack (`Vec<Value>`)
- [x] Op budget counter: default 10,000 ops, configurable via `--ops-limit`
- [x] `RuntimeError::Timeout` on budget exhaustion
- [x] `NativeRegistry` trait object for dispatching `CALL_NATIVE` instructions
- [x] `CliRegistry` with built-in `native.log` implementation

### CLI additions
- [x] `cortex run <file>` — compile + execute in one step
- [x] `cortex compile <file> --emit=asm` — print text bytecode
- [x] `cortex compile <file> -o out.ctxb` — write binary file

### Phase 2 Verification
- [x] `cortex run math.ctx` → prints `42`
- [x] `cortex compile math.ctx --emit=asm` → human-readable bytecode listing
- [x] `--ops-limit=2` → `error[R001]: execution budget exhausted`

---

## Phase 3: Native Bridging & IPC (Weeks 7–9)
> **Milestone:** Browser button → Cortex script → reads local file → desktop notification.

### Go Daemon (`cortex-daemon/`)
- [x] WebSocket server on `ws://127.0.0.1:9911`
- [x] JSON-RPC message handling: `ExecuteCortex { script, allow[] }` → result
- [x] Cryptographic token handshake (first-connect token, stored by PWA in `localStorage`)
- [x] API registry wired to OS:
  - [x] `native.log` → stdout
  - [x] `native.fs.read` → `os.ReadFile`
  - [x] `native.notify` → desktop notification (`beeep`)
- [x] Call `cortex-engine` binary as subprocess (AST interpreter approach — no CGo)
- [x] Unit tests for IPC message parsing and registry dispatch

### Rust Shell (`cortex-shell/`)
- [x] Set up Tauri WRY crate for cross-platform WebView
- [x] Embed the Svelte PWA bundle into the shell (runtime: dist/index.html, fallback placeholder)
- [x] Pass WebSocket traffic through to the Go daemon on loopback (WebView handles WS directly)

### Svelte PWA (`cortex-pwa/`)
- [x] Scaffold with `vite` + `@sveltejs/kit` + TypeScript
- [x] `src/lib/cortex-client.ts` — typed SDK wrapping WebSocket
  - [x] `CortexClient.connect(url)` with token handshake
  - [x] `CortexClient.execute(script, { allow[] })` → `Promise<Result>`
- [x] Demo page: "Read File" button triggers `native.fs.read`
- [x] Graceful degradation: show "Install TPT Core" banner when daemon unreachable

### Phase 3 Verification
- [x] Start daemon: `go run ./cortex-daemon --cortex-bin=./target/debug/cortex.exe`
- [x] WebSocket end-to-end: browser → Cortex script → file read → logs returned
- [ ] Open PWA in browser, click button → `~/test.txt` contents + desktop notification (manual)

---

## Phase 4: Pulse Daemon & Android (Weeks 10–12)
> **Milestone:** GPS logged to SQLite every 30s while Chrome is fully closed; syncs on reconnect.

### Go Daemon — Background Scheduler
- [x] Integrate `robfig/cron` for periodic task scheduling (`scheduler/scheduler.go`)
- [x] SQLite persistence of scheduled tasks via `modernc.org/sqlite` (pure Go, no CGO; `db/db.go`)
- [x] Wire `native.db.append` and `native.db.query` to SQLite
- [x] Wire `native.http.post` for background sync
- [x] System tray app (`getlantern/systray`) for Windows/macOS/Linux (`tray/tray.go`)

### Permission Manifest System
- [x] Define `cortex.manifest.json` schema (allowed APIs per app origin) — see `cortex.manifest.json`
- [x] Load manifest at daemon startup (`manifest/manifest.go`, `--manifest` flag)
- [x] Enforce at runtime (NativeRegistry checks `manifest.IsAllowed` before every call)
- [x] Enforce at compile time (Semantic Analyzer) — `cortex compile <file> --manifest cortex.manifest.json`

### Android Companion App (`cortex-android/`)
- [x] Kotlin project scaffold (Kotlin DSL Gradle)
- [x] System Dashboard UI (`DashboardActivity.kt`) — prevents OS from aggressively killing the app
- [x] 3-step onboarding wizard:
  - [x] Step 1: Welcome screen (`WelcomeFragment.kt`)
  - [x] Step 2: Battery optimization bypass (`BatteryFragment.kt`)
  - [x] Step 3: Foreground service notification permission (`PermissionFragment.kt`)
- [x] `ForegroundService` + `WorkManager` for background task execution
- [ ] Build `cortex-engine` to ARM64 `.so` via `cargo-ndk` — run: `cargo ndk -t arm64-v8a build --release`
- [x] JNI bridge: `CortexEngine.kt` calling `cortex_compile()` (`src/ffi.rs` + `cdylib` crate type)
- [x] Local WebSocket server on `127.0.0.1:9911` (`WebSocketServer.kt`, matches desktop protocol)
- [x] Cryptographic token handshake (UUID stored in SharedPreferences, sent on connect)
- [x] `native.location` wired to `FusedLocationProviderClient` (`LocationRepository.kt`)
- [x] `native.db` wired to local SQLite via Room (`CortexDatabase.kt`)

### Demo PWA (GPS Timesheet)
- [x] Update `cortex-pwa/` with GPS timesheet demo UI
- [x] Cortex script: subscribe to location, append to DB, sync on reconnect (`examples/gps_timesheet.ctx` + `native.location.current()` in daemon registry)
- [ ] Test: lock screen → verify DB entries accumulate → airplane mode off → verify sync

### Distribution
- [x] Apache 2.0 `LICENSE` file (Copyright 2026 TPT Solutions)
- [ ] Auto-updater: ping GitHub Releases API on launch, download + prompt install
- [ ] `tpt.dev/install` landing page with OS detection + APK download
- [ ] Obtainium-compatible GitHub Release format
- [ ] "Add to Obtainium" button on landing page

### Phase 4 Verification
- [ ] Install APK on Android device (no custom ROM)
- [ ] Open PWA in Chrome, start GPS tracking
- [ ] Lock screen + close Chrome
- [ ] Wait 2 minutes
- [ ] Reopen — verify SQLite has continuous location entries
- [ ] Enable airplane mode → disable → verify sync completes

---

## Post-MVP / Future Extensions
- [x] Wasm compilation target (compile Cortex AST → WebAssembly) — `cortex compile <file> --emit=wat|wasm`
- [x] Visual scripting node editor outputting valid Cortex code — `cortex-pwa/src/routes/nodes/+page.svelte`
- [x] Distributed Cortex (serialize + execute scripts on edge servers) — HTTP `POST /execute` in daemon
- [x] Cortex Language Server Protocol (LSP) for IDE support — `cortex-lsp` crate (`cortex-lsp` binary)
