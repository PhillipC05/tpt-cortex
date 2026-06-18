use std::collections::HashMap;
use crate::ast::*;
use super::chunk::{Chunk, Instruction};

/// Compile a parsed, type-checked program into a list of Chunks (one per task).
pub fn compile_program(program: &Program) -> Vec<Chunk> {
    program.tasks.iter().map(compile_task).collect()
}

fn compile_task(task: &Task) -> Chunk {
    let mut c = TaskCompiler::new(&task.name.name);

    // Allocate parameter slots first
    for param in &task.params {
        c.declare_local(&param.name.name);
    }

    c.compile_block(&task.body);

    // Ensure every path ends with Halt
    c.chunk.emit(Instruction::Halt);

    c.chunk.local_count = c.next_slot;
    c.chunk
}

// ── TaskCompiler ──────────────────────────────────────────────────────────────

struct TaskCompiler {
    chunk: Chunk,
    /// Stack of scopes; each scope maps name → slot.
    scopes: Vec<HashMap<String, usize>>,
    next_slot: usize,
}

impl TaskCompiler {
    fn new(task_name: &str) -> Self {
        Self {
            chunk: Chunk::new(task_name),
            scopes: vec![HashMap::new()],
            next_slot: 0,
        }
    }

    // ── Scope helpers ─────────────────────────────────────────────────────

    fn push_scope(&mut self) { self.scopes.push(HashMap::new()); }
    fn pop_scope(&mut self)  { self.scopes.pop(); }

    fn declare_local(&mut self, name: &str) -> usize {
        let slot = self.next_slot;
        self.next_slot += 1;
        self.scopes.last_mut().unwrap().insert(name.to_string(), slot);
        if self.chunk.local_names.len() <= slot {
            self.chunk.local_names.resize(slot + 1, String::new());
        }
        self.chunk.local_names[slot] = name.to_string();
        slot
    }

    fn resolve_local(&self, name: &str) -> Option<usize> {
        for scope in self.scopes.iter().rev() {
            if let Some(&slot) = scope.get(name) {
                return Some(slot);
            }
        }
        None
    }

    // ── Block / statement compilation ─────────────────────────────────────

    fn compile_block(&mut self, block: &[Stmt]) {
        self.push_scope();
        for stmt in block {
            self.compile_stmt(stmt);
        }
        self.pop_scope();
    }

    fn compile_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(s)    => self.compile_let(s),
            Stmt::If(s)     => self.compile_if(s),
            Stmt::Return(s) => self.compile_return(s),
            Stmt::Expr(s)   => {
                self.compile_expr(&s.expr);
                self.chunk.emit(Instruction::Pop);
            }
        }
    }

    fn compile_let(&mut self, stmt: &LetStmt) {
        self.compile_expr(&stmt.value);
        let slot = self.declare_local(&stmt.name.name);
        self.chunk.emit(Instruction::Store(slot));
    }

    fn compile_if(&mut self, stmt: &IfStmt) {
        // Compile condition
        self.compile_expr(&stmt.condition);

        // Emit JumpIfFalse with placeholder target
        let jump_to_else = self.chunk.emit(Instruction::JumpIfFalse(0));

        // Compile then-block
        self.compile_block(&stmt.then_block);

        match &stmt.else_branch {
            None => {
                // No else: patch jump to land just after the then-block
                let after_then = self.chunk.instructions.len();
                self.chunk.patch_jump(jump_to_else, after_then);
            }
            Some(ElseBranch::Block(blk)) => {
                // Jump over the else-block at end of then-block
                let jump_over_else = self.chunk.emit(Instruction::Jump(0));
                let else_start = self.chunk.instructions.len();
                self.chunk.patch_jump(jump_to_else, else_start);
                self.compile_block(blk);
                let after_else = self.chunk.instructions.len();
                self.chunk.patch_jump(jump_over_else, after_else);
            }
            Some(ElseBranch::ElseIf(nested)) => {
                let jump_over_else = self.chunk.emit(Instruction::Jump(0));
                let else_start = self.chunk.instructions.len();
                self.chunk.patch_jump(jump_to_else, else_start);
                self.compile_if(nested);
                let after_else = self.chunk.instructions.len();
                self.chunk.patch_jump(jump_over_else, after_else);
            }
        }
    }

    fn compile_return(&mut self, stmt: &ReturnStmt) {
        match &stmt.value {
            Some(v) => self.compile_expr(v),
            None    => { self.chunk.emit(Instruction::PushVoid); }
        }
        self.chunk.emit(Instruction::Return);
    }

    // ── Expression compilation ────────────────────────────────────────────

    fn compile_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(l)     => self.compile_literal(l),
            Expr::Ident(id)      => self.compile_ident(id),
            Expr::Binary(b)      => self.compile_binary(b),
            Expr::Unary(u)       => self.compile_unary(u),
            Expr::Call(c)        => self.compile_call(c),
            Expr::NativeCall(nc) => self.compile_native_call(nc),
            Expr::Index(i)       => self.compile_index(i),
        }
    }

    fn compile_literal(&mut self, lit: &Literal) {
        match &lit.kind {
            LitKind::Int(n)  => { self.chunk.emit(Instruction::PushI32(*n as i32)); }
            LitKind::Float(f) => { self.chunk.emit(Instruction::PushF64(*f)); }
            LitKind::Str(s)  => {
                let idx = self.chunk.add_string(s.clone());
                self.chunk.emit(Instruction::PushStr(idx));
            }
            LitKind::Bool(b) => { self.chunk.emit(Instruction::PushBool(*b)); }
        }
    }

    fn compile_ident(&mut self, id: &Ident) {
        let slot = self.resolve_local(&id.name)
            .unwrap_or_else(|| panic!("compiler: undefined '{}' — type checker should have caught this", id.name));
        self.chunk.emit(Instruction::Load(slot));
    }

    fn compile_binary(&mut self, b: &BinaryExpr) {
        self.compile_expr(&b.left);
        self.compile_expr(&b.right);
        let instr = match b.op {
            BinOp::Add   => Instruction::Add,
            BinOp::Sub   => Instruction::Sub,
            BinOp::Mul   => Instruction::Mul,
            BinOp::Div   => Instruction::Div,
            BinOp::Eq    => Instruction::Eq,
            BinOp::NotEq => Instruction::NotEq,
            BinOp::Lt    => Instruction::Lt,
            BinOp::LtEq  => Instruction::LtEq,
            BinOp::Gt    => Instruction::Gt,
            BinOp::GtEq  => Instruction::GtEq,
            BinOp::And   => Instruction::And,
            BinOp::Or    => Instruction::Or,
        };
        self.chunk.emit(instr);
    }

    fn compile_unary(&mut self, u: &UnaryExpr) {
        self.compile_expr(&u.operand);
        match u.op {
            UnaryOp::Neg => { self.chunk.emit(Instruction::Neg); }
            UnaryOp::Not => { self.chunk.emit(Instruction::Not); }
        }
    }

    fn compile_call(&mut self, c: &CallExpr) {
        // User-defined task calls not yet supported (Phase 3+).
        // Emit a no-op Void push so the stack stays consistent.
        for arg in &c.args { self.compile_expr(arg); }
        for _ in &c.args   { self.chunk.emit(Instruction::Pop); }
        self.chunk.emit(Instruction::PushVoid);
    }

    fn compile_native_call(&mut self, nc: &NativeCallExpr) {
        let argc = nc.args.len();
        // Push args left-to-right; VM pops them right-to-left.
        for arg in &nc.args {
            self.compile_expr(arg);
        }
        let api_id = self.chunk.add_native(nc.qualified_name());
        self.chunk.emit(Instruction::CallNative(api_id, argc));
    }

    fn compile_index(&mut self, i: &IndexExpr) {
        // Index expressions not fully implemented in Phase 2 VM — push Void.
        self.compile_expr(&i.object);
        self.compile_expr(&i.index);
        self.chunk.emit(Instruction::Pop); // index
        self.chunk.emit(Instruction::Pop); // object
        self.chunk.emit(Instruction::PushVoid);
    }
}
