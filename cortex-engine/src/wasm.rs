/// WebAssembly Text Format (.wat) emitter for Cortex programs.
///
/// Compiles a type-checked Cortex AST directly to WAT, bypassing the bytecode VM.
/// Output can be assembled to binary Wasm with `wat2wasm` (wabt) or the `wat` crate.
///
/// Type mapping:
///   i32, bool  → i32
///   f64        → f64
///   string     → i32 (byte offset into linear memory)
///   void       → no Wasm result type
///   list, map  → i32 (not deeply supported; placeholder)
///
/// Native calls become Wasm imports under the "native" module namespace.
/// String literals are stored null-terminated in the data section.
use std::collections::{HashMap, HashSet};
use crate::ast::*;

// ── Wasm value types ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum Wt { I32, F64 }

impl Wt {
    fn s(self) -> &'static str {
        match self { Wt::I32 => "i32", Wt::F64 => "f64" }
    }
}

fn ty_to_wt(ty: &TypeKind) -> Option<Wt> {
    match ty {
        TypeKind::I32  => Some(Wt::I32),
        TypeKind::F64  => Some(Wt::F64),
        TypeKind::Bool => Some(Wt::I32),
        TypeKind::Str  => Some(Wt::I32),  // pointer into linear memory
        TypeKind::Void => None,
        TypeKind::List(_) | TypeKind::Map(_, _) => Some(Wt::I32),
    }
}

// ── Module-level context ─────────────────────────────────────────────────────

#[derive(Default)]
struct ModuleCtx {
    /// Ordered native imports: (cortex_qualified_name, param_types, has_result).
    native_imports: Vec<NativeImport>,
    /// Interned string literals, in insertion order.
    strings: Vec<String>,
}

struct NativeImport {
    cortex_name: String,  // e.g. "native.log"
    params:      Vec<Wt>,
    has_result:  bool,
}

impl ModuleCtx {
    /// Register a native call's signature on first encounter. Later call sites
    /// with the same name are ignored (first-wins).
    fn register_native(&mut self, name: &str, params: Vec<Wt>, has_result: bool) {
        if !self.native_imports.iter().any(|n| n.cortex_name == name) {
            self.native_imports.push(NativeImport {
                cortex_name: name.to_string(),
                params,
                has_result,
            });
        }
    }

    /// Intern a string literal and return its byte offset in the data section.
    fn intern_string(&mut self, s: &str) -> u32 {
        let mut offset: u32 = 0;
        for existing in &self.strings {
            if existing == s { return offset; }
            offset += existing.len() as u32 + 1; // +1 for null terminator
        }
        self.strings.push(s.to_string());
        offset
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Compile a type-checked Cortex program to a WAT module string.
pub fn compile_to_wat(program: &Program) -> String {
    let mut ctx = ModuleCtx::default();

    // Build return-type map so call expressions resolve correctly (especially void tasks).
    let task_types: HashMap<String, Option<Wt>> = program.tasks.iter()
        .map(|t| (t.name.name.clone(), ty_to_wt(&t.return_ty.kind)))
        .collect();

    let func_bodies: Vec<String> = program.tasks.iter()
        .map(|task| compile_task(task, &mut ctx, &task_types))
        .collect();

    build_module_string(&ctx, &func_bodies)
}

/// Compile a type-checked Cortex program to a binary `.wasm` module.
///
/// The WAT text is first generated, then assembled using the `wat` crate.
/// Returns an error string if the generated WAT fails to parse (should not
/// happen unless there is a bug in the WAT emitter).
pub fn compile_to_wasm(program: &Program) -> Result<Vec<u8>, String> {
    let text = compile_to_wat(program);
    wat::parse_str(&text).map_err(|e| format!("wasm assembly error: {e}"))
}

// ── Task compilation ──────────────────────────────────────────────────────────

fn compile_task(task: &Task, ctx: &mut ModuleCtx, task_types: &HashMap<String, Option<Wt>>) -> String {
    let name = &task.name.name;

    // WAT parameter list from Cortex params
    let params_str: String = task.params.iter()
        .filter_map(|p| ty_to_wt(&p.ty.kind).map(|wt| format!(" (param ${} {})", p.name.name, wt.s())))
        .collect();

    // WAT result type (omitted for void)
    let result_str: String = ty_to_wt(&task.return_ty.kind)
        .map(|wt| format!(" (result {})", wt.s()))
        .unwrap_or_default();

    // Build type environment from parameters
    let mut type_env: HashMap<String, Wt> = task.params.iter()
        .filter_map(|p| ty_to_wt(&p.ty.kind).map(|wt| (p.name.name.clone(), wt)))
        .collect();

    // Collect all `let` locals declared in this task body (scoped uniqueness)
    let mut locals: Vec<(String, Wt)> = Vec::new();
    let mut seen_locals: HashSet<String> = HashSet::new();
    collect_locals(&task.body, &mut locals, &mut seen_locals);
    for (lname, wt) in &locals {
        type_env.insert(lname.clone(), *wt);
    }

    // Compile the body
    let mut emitter = BodyEmitter { type_env, task_types, ctx, buf: String::new(), indent: 2 };
    let body_clone = task.body.clone();
    emitter.compile_block(&body_clone);

    // Assemble function text
    let mut out = format!("  (func ${name} (export \"{name}\"){params_str}{result_str}\n");
    for (lname, wt) in &locals {
        out += &format!("    (local ${} {})\n", lname, wt.s());
    }
    out += &emitter.buf;
    out += "  )\n";
    out
}

/// Recursively collect all `let` bindings from a block (one declaration per
/// unique name; shadowing in nested scopes uses the outer declaration's slot).
fn collect_locals(block: &[Stmt], out: &mut Vec<(String, Wt)>, seen: &mut HashSet<String>) {
    for stmt in block {
        match stmt {
            Stmt::Let(s) => {
                if let Some(wt) = ty_to_wt(&s.ty.kind) {
                    if seen.insert(s.name.name.clone()) {
                        out.push((s.name.name.clone(), wt));
                    }
                }
            }
            Stmt::If(s) => {
                collect_locals(&s.then_block, out, seen);
                match &s.else_branch {
                    Some(ElseBranch::Block(b)) => collect_locals(b, out, seen),
                    Some(ElseBranch::ElseIf(nested)) => {
                        collect_locals(&nested.then_block, out, seen);
                        // deeper else-if chains handled via recursive stmt traversal
                        collect_else_if_locals(nested, out, seen);
                    }
                    None => {}
                }
            }
            _ => {}
        }
    }
}

fn collect_else_if_locals(s: &IfStmt, out: &mut Vec<(String, Wt)>, seen: &mut HashSet<String>) {
    match &s.else_branch {
        Some(ElseBranch::Block(b)) => collect_locals(b, out, seen),
        Some(ElseBranch::ElseIf(nested)) => {
            collect_locals(&nested.then_block, out, seen);
            collect_else_if_locals(nested, out, seen);
        }
        None => {}
    }
}

// ── Body emitter ──────────────────────────────────────────────────────────────

struct BodyEmitter<'a> {
    type_env:   HashMap<String, Wt>,
    /// Return types of all user-defined tasks; `None` means void.
    task_types: &'a HashMap<String, Option<Wt>>,
    ctx:        &'a mut ModuleCtx,
    buf:        String,
    indent:     usize,
}

impl<'a> BodyEmitter<'a> {
    fn line(&mut self, s: &str) {
        for _ in 0..self.indent { self.buf += "  "; }
        self.buf += s;
        self.buf += "\n";
    }

    fn compile_block(&mut self, block: &[Stmt]) {
        for stmt in block {
            let stmt = stmt.clone();
            self.compile_stmt(&stmt);
        }
    }

    fn compile_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(s) => {
                let value = s.value.clone();
                let name  = s.name.name.clone();
                self.compile_expr(&value);
                self.line(&format!("local.set ${name}"));
            }

            Stmt::If(s) => {
                let cond         = s.condition.clone();
                let then_block   = s.then_block.clone();
                let else_branch  = s.else_branch.clone();

                self.compile_expr(&cond);
                self.line("if");
                self.indent += 1;
                self.compile_block(&then_block);
                self.indent -= 1;

                match else_branch {
                    None => {}
                    Some(ElseBranch::Block(blk)) => {
                        self.line("else");
                        self.indent += 1;
                        self.compile_block(&blk);
                        self.indent -= 1;
                    }
                    Some(ElseBranch::ElseIf(nested)) => {
                        self.line("else");
                        self.indent += 1;
                        self.compile_stmt(&Stmt::If(*nested));
                        self.indent -= 1;
                    }
                }
                self.line("end");
            }

            Stmt::Return(s) => {
                if let Some(v) = &s.value.clone() {
                    self.compile_expr(v);
                }
                self.line("return");
            }

            Stmt::Expr(s) => {
                let expr = s.expr.clone();
                let produces_value = self.type_of_expr(&expr).is_some();
                self.compile_expr(&expr);
                if produces_value {
                    self.line("drop");
                }
            }
        }
    }

    fn compile_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(l) => self.compile_literal(l),
            Expr::Ident(id)  => self.line(&format!("local.get ${}", id.name)),

            Expr::Binary(b) => {
                let left     = b.left.clone();
                let right    = b.right.clone();
                let op       = b.op.clone();
                let left_ty  = self.type_of_expr(&left).unwrap_or(Wt::I32);
                self.compile_expr(&left);
                self.compile_expr(&right);
                self.line(bin_op_instr(&op, left_ty));
            }

            Expr::Unary(u) => {
                let operand = u.operand.clone();
                let op      = u.op.clone();
                let ty      = self.type_of_expr(&operand).unwrap_or(Wt::I32);
                self.compile_expr(&operand);
                match (op, ty) {
                    (UnaryOp::Neg, Wt::I32) => {
                        self.line("i32.const -1");
                        self.line("i32.mul");
                    }
                    (UnaryOp::Neg, Wt::F64) => self.line("f64.neg"),
                    (UnaryOp::Not, _) => self.line("i32.eqz"),
                }
            }

            Expr::Call(c) => {
                let callee = c.callee.name.clone();
                let args   = c.args.clone();
                for arg in &args { self.compile_expr(arg); }
                self.line(&format!("call ${callee}"));
            }

            Expr::NativeCall(nc) => {
                let qname   = nc.qualified_name();
                let args    = nc.args.clone();
                let arg_tys: Vec<Wt> = args.iter()
                    .map(|a| self.type_of_expr(a).unwrap_or(Wt::I32))
                    .collect();
                // Register import on first encounter (first call-site wins)
                self.ctx.register_native(&qname, arg_tys, true);
                for arg in &args { self.compile_expr(arg); }
                let wasm_fn = native_wasm_name(&qname);
                self.line(&format!("call ${wasm_fn}"));
            }

            Expr::Index(_) => {
                // Not supported in initial Wasm target
                self.line("i32.const 0  ;; unsupported: index expression");
            }
        }
    }

    fn compile_literal(&mut self, lit: &Literal) {
        match &lit.kind {
            LitKind::Int(n)  => self.line(&format!("i32.const {n}")),
            LitKind::Float(f) => self.line(&format!("f64.const {f}")),
            LitKind::Bool(b) => self.line(&format!("i32.const {}", if *b { 1 } else { 0 })),
            LitKind::Str(s)  => {
                let s = s.clone();
                let offset = self.ctx.intern_string(&s);
                self.line(&format!("i32.const {offset}  ;; {:?}", s));
            }
        }
    }

    fn type_of_expr(&self, expr: &Expr) -> Option<Wt> {
        match expr {
            Expr::Literal(l) => match &l.kind {
                LitKind::Int(_) | LitKind::Bool(_) | LitKind::Str(_) => Some(Wt::I32),
                LitKind::Float(_) => Some(Wt::F64),
            },
            Expr::Ident(id) => self.type_env.get(&id.name).copied(),
            Expr::Binary(b) => match b.op {
                BinOp::Eq | BinOp::NotEq |
                BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq |
                BinOp::And | BinOp::Or => Some(Wt::I32),
                _ => self.type_of_expr(&b.left),
            },
            Expr::Unary(u) => match u.op {
                UnaryOp::Not => Some(Wt::I32),
                UnaryOp::Neg => self.type_of_expr(&u.operand),
            },
            Expr::Call(c) => self.task_types.get(&c.callee.name).copied().flatten(),
            // Native calls are declared as returning i32; host must conform.
            Expr::NativeCall(_) | Expr::Index(_) => Some(Wt::I32),
        }
    }
}

// ── Module assembly ───────────────────────────────────────────────────────────

fn build_module_string(ctx: &ModuleCtx, func_bodies: &[String]) -> String {
    let mut out = String::from("(module\n");

    // Import declarations for native calls
    for ni in &ctx.native_imports {
        let wasm_fn = native_wasm_name(&ni.cortex_name);
        let params: String = ni.params.iter()
            .map(|wt| format!(" {}", wt.s()))
            .collect();
        let result = if ni.has_result { " (result i32)" } else { "" };
        out += &format!(
            "  (import \"native\" \"{}\" (func ${wasm_fn} (param{params}){result}))\n",
            ni.cortex_name
        );
    }

    // Linear memory for string storage (only if strings were interned)
    if !ctx.strings.is_empty() {
        out += "  (memory (export \"memory\") 1)\n";
        let mut offset: u32 = 0;
        for s in &ctx.strings {
            let escaped = escape_wat_str(s);
            let len = s.len();
            out += &format!(
                "  (data (i32.const {offset}) \"{escaped}\\00\")  ;; len={len}\n"
            );
            offset += len as u32 + 1;
        }
    }

    // Function bodies
    for body in func_bodies {
        out += body;
    }

    out += ")\n";
    out
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Map a Cortex native name like "native.fs.read" to a WAT identifier like
/// "native_fs_read" (dots are not valid in WAT identifiers outside of quotes).
fn native_wasm_name(cortex_name: &str) -> String {
    cortex_name.replace('.', "_")
}

fn bin_op_instr(op: &BinOp, ty: Wt) -> &'static str {
    match (op, ty) {
        (BinOp::Add,   Wt::I32) => "i32.add",
        (BinOp::Add,   Wt::F64) => "f64.add",
        (BinOp::Sub,   Wt::I32) => "i32.sub",
        (BinOp::Sub,   Wt::F64) => "f64.sub",
        (BinOp::Mul,   Wt::I32) => "i32.mul",
        (BinOp::Mul,   Wt::F64) => "f64.mul",
        (BinOp::Div,   Wt::I32) => "i32.div_s",
        (BinOp::Div,   Wt::F64) => "f64.div",
        (BinOp::Eq,    Wt::I32) => "i32.eq",
        (BinOp::Eq,    Wt::F64) => "f64.eq",
        (BinOp::NotEq, Wt::I32) => "i32.ne",
        (BinOp::NotEq, Wt::F64) => "f64.ne",
        (BinOp::Lt,    Wt::I32) => "i32.lt_s",
        (BinOp::Lt,    Wt::F64) => "f64.lt",
        (BinOp::LtEq,  Wt::I32) => "i32.le_s",
        (BinOp::LtEq,  Wt::F64) => "f64.le",
        (BinOp::Gt,    Wt::I32) => "i32.gt_s",
        (BinOp::Gt,    Wt::F64) => "f64.gt",
        (BinOp::GtEq,  Wt::I32) => "i32.ge_s",
        (BinOp::GtEq,  Wt::F64) => "f64.ge",
        (BinOp::And,   _)       => "i32.and",
        (BinOp::Or,    _)       => "i32.or",
    }
}

/// Escape a Rust string for use inside a WAT double-quoted string literal.
fn escape_wat_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'"'  => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\0a"),
            b'\r' => out.push_str("\\0d"),
            b'\t' => out.push_str("\\09"),
            0x20..=0x7e => out.push(byte as char),
            _     => { let _ = std::fmt::Write::write_fmt(&mut out, format_args!("\\{:02x}", byte)); }
        }
    }
    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compile, checker::PermissionManifest};

    fn wat(src: &str) -> String {
        let manifest = PermissionManifest::new(vec![
            "native.log".to_string(),
            "native.fs.read".to_string(),
        ]);
        let ast = compile(src, &manifest).expect("compile failed");
        compile_to_wat(&ast)
    }

    #[test]
    fn test_simple_i32_task() {
        let w = wat("task add(a: i32, b: i32) -> i32 { return a + b; }");
        assert!(w.contains("(func $add"));
        assert!(w.contains("(param $a i32)"));
        assert!(w.contains("(param $b i32)"));
        assert!(w.contains("(result i32)"));
        assert!(w.contains("local.get $a"));
        assert!(w.contains("local.get $b"));
        assert!(w.contains("i32.add"));
        assert!(w.contains("return"));
    }

    #[test]
    fn test_f64_arithmetic() {
        let w = wat("task square(x: f64) -> f64 { return x * x; }");
        assert!(w.contains("(result f64)"));
        assert!(w.contains("f64.mul"));
    }

    #[test]
    fn test_bool_literal() {
        let w = wat("task yes() -> bool { return true; }");
        assert!(w.contains("(result i32)"));
        assert!(w.contains("i32.const 1"));
    }

    #[test]
    fn test_let_and_if() {
        let w = wat(
            "task abs(x: i32) -> i32 {\
               let r: i32 = x;\
               if x < 0 { return 0 - x; }\
               return r;\
             }",
        );
        assert!(w.contains("(local $r i32)"));
        assert!(w.contains("i32.lt_s"));
        assert!(w.contains("if"));
        assert!(w.contains("end"));
    }

    #[test]
    fn test_void_task_no_result() {
        let w = wat("task noop() -> void { }");
        assert!(!w.contains("(result"), "void task should have no result type");
    }

    #[test]
    fn test_string_literal_in_data_section() {
        let w = wat("task greet() -> void { native.log(\"hello\"); }");
        assert!(w.contains("(memory"));
        assert!(w.contains("(data"));
        assert!(w.contains("\"hello\\00\""));
        assert!(w.contains("(import \"native\" \"native.log\""));
    }

    #[test]
    fn test_export_name() {
        let w = wat("task compute() -> i32 { return 42; }");
        assert!(w.contains("(export \"compute\")"));
    }
}
