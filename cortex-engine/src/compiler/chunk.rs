/// A single bytecode instruction in the stack-based Cortex VM.
#[derive(Debug, Clone)]
pub enum Instruction {
    // ── Push literals ────────────────────────────
    PushI32(i32),
    PushF64(f64),
    PushStr(usize),   // index into Chunk::string_table
    PushBool(bool),
    PushVoid,

    // ── Local variables ──────────────────────────
    Load(usize),      // push locals[slot] onto stack
    Store(usize),     // pop top of stack → locals[slot]
    Pop,              // discard top of stack (for ExprStmt)

    // ── Arithmetic ───────────────────────────────
    Add, Sub, Mul, Div, Neg,

    // ── Comparison → bool ────────────────────────
    Eq, NotEq, Lt, LtEq, Gt, GtEq,

    // ── Logical ──────────────────────────────────
    And, Or, Not,

    // ── Native calls ─────────────────────────────
    /// CallNative(api_id, argc): pop argc args, call native_table[api_id], push result
    CallNative(usize, usize),

    // ── Control flow ─────────────────────────────
    Jump(usize),          // unconditional jump to absolute index
    JumpIfFalse(usize),   // pop bool, jump if false
    Return,               // pop top and return it (or return Void)
    Halt,
}

/// A compiled task: instructions + data tables used during execution.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub task_name: String,
    pub instructions: Vec<Instruction>,
    pub string_table: Vec<String>,
    /// Qualified native API names, indexed by api_id used in CallNative.
    pub native_table: Vec<String>,
    pub local_count: usize,
    /// Debug info: slot index → variable name (populated during compilation).
    pub local_names: Vec<String>,
}

impl Chunk {
    pub fn new(task_name: impl Into<String>) -> Self {
        Self {
            task_name: task_name.into(),
            instructions: Vec::new(),
            string_table: Vec::new(),
            native_table: Vec::new(),
            local_count: 0,
            local_names: Vec::new(),
        }
    }

    /// Intern a string and return its table index.
    pub fn add_string(&mut self, s: impl Into<String>) -> usize {
        let s = s.into();
        if let Some(idx) = self.string_table.iter().position(|x| x == &s) {
            return idx;
        }
        let idx = self.string_table.len();
        self.string_table.push(s);
        idx
    }

    /// Intern a native API name and return its table index.
    pub fn add_native(&mut self, api: impl Into<String>) -> usize {
        let api = api.into();
        if let Some(idx) = self.native_table.iter().position(|x| x == &api) {
            return idx;
        }
        let idx = self.native_table.len();
        self.native_table.push(api);
        idx
    }

    pub fn emit(&mut self, instr: Instruction) -> usize {
        let idx = self.instructions.len();
        self.instructions.push(instr);
        idx
    }

    /// Patch a previously-emitted Jump or JumpIfFalse with the real target offset.
    pub fn patch_jump(&mut self, idx: usize, target: usize) {
        match &mut self.instructions[idx] {
            Instruction::Jump(t) | Instruction::JumpIfFalse(t) => *t = target,
            _ => panic!("patch_jump called on non-jump instruction"),
        }
    }
}
