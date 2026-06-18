use crate::compiler::chunk::{Chunk, Instruction};
use super::{NativeRegistry, RuntimeError, Value};

pub struct Vm<'r> {
    chunk: Chunk,
    ip: usize,
    stack: Vec<Value>,
    locals: Vec<Value>,
    ops_remaining: u64,
    registry: &'r mut dyn NativeRegistry,
}

impl<'r> Vm<'r> {
    pub fn new(chunk: Chunk, ops_limit: u64, registry: &'r mut dyn NativeRegistry) -> Self {
        let local_count = chunk.local_count;
        Self {
            chunk,
            ip: 0,
            stack: Vec::with_capacity(64),
            locals: vec![Value::Void; local_count.max(1)],
            ops_remaining: ops_limit,
            registry,
        }
    }

    pub fn run(mut self) -> Result<Value, RuntimeError> {
        loop {
            if self.ops_remaining == 0 {
                return Err(RuntimeError::Timeout {
                    ops: self.ip as u64,
                });
            }
            self.ops_remaining -= 1;

            let instr = self.chunk.instructions[self.ip].clone();
            self.ip += 1;

            match instr {
                // ── Push ──────────────────────────────────────────────────
                Instruction::PushI32(n)  => self.stack.push(Value::I32(n)),
                Instruction::PushF64(f)  => self.stack.push(Value::F64(f)),
                Instruction::PushStr(i)  => self.stack.push(Value::Str(self.chunk.string_table[i].clone())),
                Instruction::PushBool(b) => self.stack.push(Value::Bool(b)),
                Instruction::PushVoid    => self.stack.push(Value::Void),

                // ── Locals ────────────────────────────────────────────────
                Instruction::Load(slot) => {
                    let v = self.locals.get(slot)
                        .cloned()
                        .ok_or(RuntimeError::StackUnderflow)?;
                    self.stack.push(v);
                }
                Instruction::Store(slot) => {
                    let v = self.pop()?;
                    if slot >= self.locals.len() {
                        self.locals.resize(slot + 1, Value::Void);
                    }
                    self.locals[slot] = v;
                }
                Instruction::Pop => { self.pop()?; }

                // ── Arithmetic ────────────────────────────────────────────
                Instruction::Add => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(match (a, b) {
                        (Value::I32(a), Value::I32(b)) => Value::I32(a.wrapping_add(b)),
                        (Value::F64(a), Value::F64(b)) => Value::F64(a + b),
                        (Value::Str(a), Value::Str(b)) => Value::Str(a + &b),
                        (a, _b) => return Err(RuntimeError::TypeError {
                            expected: "matching numeric or string",
                            found: a.type_name(),
                        }),
                    });
                }
                Instruction::Sub => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(match (a, b) {
                        (Value::I32(a), Value::I32(b)) => Value::I32(a.wrapping_sub(b)),
                        (Value::F64(a), Value::F64(b)) => Value::F64(a - b),
                        (a, _) => return Err(RuntimeError::TypeError { expected: "i32 or f64", found: a.type_name() }),
                    });
                }
                Instruction::Mul => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(match (a, b) {
                        (Value::I32(a), Value::I32(b)) => Value::I32(a.wrapping_mul(b)),
                        (Value::F64(a), Value::F64(b)) => Value::F64(a * b),
                        (a, _) => return Err(RuntimeError::TypeError { expected: "i32 or f64", found: a.type_name() }),
                    });
                }
                Instruction::Div => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(match (a, b) {
                        (Value::I32(a), Value::I32(b)) => {
                            if b == 0 { return Err(RuntimeError::DivisionByZero); }
                            Value::I32(a / b)
                        }
                        (Value::F64(a), Value::F64(b)) => Value::F64(a / b),
                        (a, _) => return Err(RuntimeError::TypeError { expected: "i32 or f64", found: a.type_name() }),
                    });
                }
                Instruction::Neg => {
                    let v = self.pop()?;
                    self.stack.push(match v {
                        Value::I32(n) => Value::I32(-n),
                        Value::F64(f) => Value::F64(-f),
                        other => return Err(RuntimeError::TypeError { expected: "i32 or f64", found: other.type_name() }),
                    });
                }

                // ── Comparison ────────────────────────────────────────────
                Instruction::Eq    => { let (a, b) = self.pop2()?; self.stack.push(Value::Bool(a == b)); }
                Instruction::NotEq => { let (a, b) = self.pop2()?; self.stack.push(Value::Bool(a != b)); }
                Instruction::Lt    => { let (a, b) = self.pop2()?; self.stack.push(Value::Bool(cmp_lt(a, b)?)); }
                Instruction::LtEq  => { let (a, b) = self.pop2()?; self.stack.push(Value::Bool(!cmp_lt(b.clone(), a.clone())? || a == b)); }
                Instruction::Gt    => { let (a, b) = self.pop2()?; self.stack.push(Value::Bool(cmp_lt(b, a)?)); }
                Instruction::GtEq  => { let (a, b) = self.pop2()?; self.stack.push(Value::Bool(!cmp_lt(a.clone(), b.clone())? || a == b)); }

                // ── Logical ───────────────────────────────────────────────
                Instruction::And => {
                    let (a, b) = self.pop2()?;
                    match (a, b) {
                        (Value::Bool(a), Value::Bool(b)) => self.stack.push(Value::Bool(a && b)),
                        (a, _) => return Err(RuntimeError::TypeError { expected: "bool", found: a.type_name() }),
                    }
                }
                Instruction::Or => {
                    let (a, b) = self.pop2()?;
                    match (a, b) {
                        (Value::Bool(a), Value::Bool(b)) => self.stack.push(Value::Bool(a || b)),
                        (a, _) => return Err(RuntimeError::TypeError { expected: "bool", found: a.type_name() }),
                    }
                }
                Instruction::Not => {
                    let v = self.pop()?;
                    match v {
                        Value::Bool(b) => self.stack.push(Value::Bool(!b)),
                        other => return Err(RuntimeError::TypeError { expected: "bool", found: other.type_name() }),
                    }
                }

                // ── Native call ───────────────────────────────────────────
                Instruction::CallNative(api_id, argc) => {
                    let api = self.chunk.native_table[api_id].clone();
                    // Pop args in reverse order (last arg is top of stack)
                    let mut args: Vec<Value> = (0..argc)
                        .map(|_| self.pop())
                        .collect::<Result<Vec<_>, _>>()?;
                    args.reverse(); // restore left-to-right order
                    let result = self.registry.call(&api, args)?;
                    self.stack.push(result);
                }

                // ── Control flow ──────────────────────────────────────────
                Instruction::Jump(target) => {
                    self.ip = target;
                }
                Instruction::JumpIfFalse(target) => {
                    match self.pop()? {
                        Value::Bool(false) => { self.ip = target; }
                        Value::Bool(true)  => {}
                        other => return Err(RuntimeError::TypeError { expected: "bool", found: other.type_name() }),
                    }
                }
                Instruction::Return => {
                    return Ok(self.pop().unwrap_or(Value::Void));
                }
                Instruction::Halt => {
                    return Ok(self.stack.last().cloned().unwrap_or(Value::Void));
                }
            }
        }
    }

    // ── Stack helpers ─────────────────────────────────────────────────────

    fn pop(&mut self) -> Result<Value, RuntimeError> {
        self.stack.pop().ok_or(RuntimeError::StackUnderflow)
    }

    /// Pop two values: returns (left, right) in the order they were pushed.
    fn pop2(&mut self) -> Result<(Value, Value), RuntimeError> {
        let b = self.pop()?;
        let a = self.pop()?;
        Ok((a, b))
    }
}

fn cmp_lt(a: Value, b: Value) -> Result<bool, RuntimeError> {
    match (a, b) {
        (Value::I32(a), Value::I32(b)) => Ok(a < b),
        (Value::F64(a), Value::F64(b)) => Ok(a < b),
        (a, _) => Err(RuntimeError::TypeError { expected: "i32 or f64", found: a.type_name() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::{chunk::Chunk, compiler::compile_program};
    use crate::lexer::tokenize;
    use crate::parser::parse;
    use crate::checker::{check, PermissionManifest};
    use crate::vm::registry::CliRegistry;

    fn run_src(src: &str) -> Result<Value, RuntimeError> {
        let tokens = tokenize(src).unwrap();
        let ast = parse(tokens).unwrap();
        let manifest = PermissionManifest::allow_all();
        check(&ast, &manifest).unwrap();
        let chunks = compile_program(&ast);
        let chunk = chunks.into_iter().next().expect("no chunks");
        let mut reg = CliRegistry;
        Vm::new(chunk, 10_000, &mut reg).run()
    }

    #[test]
    fn halt_returns_void_for_empty_task() {
        let v = run_src("task t() -> void {}").unwrap();
        assert_eq!(v, Value::Void);
    }

    #[test]
    fn arithmetic_i32() {
        let v = run_src("task t() -> i32 { return 6 * 7; }").unwrap();
        assert_eq!(v, Value::I32(42));
    }

    #[test]
    fn arithmetic_subtraction() {
        let v = run_src("task t() -> i32 { return 100 - 58; }").unwrap();
        assert_eq!(v, Value::I32(42));
    }

    #[test]
    fn let_and_load() {
        let v = run_src("task t() -> i32 { let x: i32 = 21; return x + x; }").unwrap();
        assert_eq!(v, Value::I32(42));
    }

    #[test]
    fn string_concat() {
        let v = run_src(r#"task t() -> string { return "hello" + " world"; }"#).unwrap();
        assert_eq!(v, Value::Str("hello world".into()));
    }

    #[test]
    fn bool_logic() {
        let v = run_src("task t() -> bool { return true && !false; }").unwrap();
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn if_taken() {
        let v = run_src("task t() -> i32 { if true { return 1; } return 2; }").unwrap();
        assert_eq!(v, Value::I32(1));
    }

    #[test]
    fn if_not_taken() {
        let v = run_src("task t() -> i32 { if false { return 1; } return 2; }").unwrap();
        assert_eq!(v, Value::I32(2));
    }

    #[test]
    fn if_else() {
        let v = run_src("task t() -> i32 { if false { return 1; } else { return 42; } }").unwrap();
        assert_eq!(v, Value::I32(42));
    }

    #[test]
    fn comparison_gt() {
        let v = run_src("task t() -> bool { return 10 > 5; }").unwrap();
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn op_budget_exhausted() {
        // A task with no return that would run forever if budget not enforced.
        // We use a very low budget on a long computation.
        let tokens = tokenize("task t() -> void { let x: i32 = 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10; }").unwrap();
        let ast = parse(tokens).unwrap();
        let manifest = PermissionManifest::allow_all();
        check(&ast, &manifest).unwrap();
        let chunks = compile_program(&ast);
        let chunk = chunks.into_iter().next().unwrap();
        let mut reg = CliRegistry;
        // Budget of 3 ops — nowhere near enough for all those additions
        let result = Vm::new(chunk, 3, &mut reg).run();
        assert!(matches!(result, Err(RuntimeError::Timeout { .. })));
    }

    #[test]
    fn division_by_zero() {
        let result = run_src("task t() -> i32 { return 1 / 0; }");
        assert!(matches!(result, Err(RuntimeError::DivisionByZero)));
    }
}
