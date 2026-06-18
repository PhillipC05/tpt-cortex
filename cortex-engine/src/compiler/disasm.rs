use super::chunk::{Chunk, Instruction};

/// Render a Chunk as human-readable text bytecode assembly.
pub fn disassemble(chunk: &Chunk) -> String {
    let mut out = String::new();

    out.push_str(&format!("== Task: {} ==\n", chunk.task_name));

    if !chunk.string_table.is_empty() {
        out.push_str("\n; String table:\n");
        for (i, s) in chunk.string_table.iter().enumerate() {
            out.push_str(&format!(";   [{i}] \"{}\"\n", s.escape_default()));
        }
    }

    if !chunk.native_table.is_empty() {
        out.push_str("\n; Native API table:\n");
        for (i, api) in chunk.native_table.iter().enumerate() {
            out.push_str(&format!(";   [{i}] {api}\n"));
        }
    }

    out.push('\n');

    for (i, instr) in chunk.instructions.iter().enumerate() {
        let comment = instr_comment(instr, chunk);
        let mnemonic = instr_mnemonic(instr);
        if comment.is_empty() {
            out.push_str(&format!("  {i:04}  {mnemonic}\n"));
        } else {
            out.push_str(&format!("  {i:04}  {mnemonic:<24}; {comment}\n"));
        }
    }

    out
}

fn instr_mnemonic(instr: &Instruction) -> String {
    match instr {
        Instruction::PushI32(n)       => format!("PUSH_I32    {n}"),
        Instruction::PushF64(f)       => format!("PUSH_F64    {f}"),
        Instruction::PushStr(i)       => format!("PUSH_STR    {i}"),
        Instruction::PushBool(b)      => format!("PUSH_BOOL   {b}"),
        Instruction::PushVoid         => "PUSH_VOID".to_string(),
        Instruction::Load(s)          => format!("LOAD        {s}"),
        Instruction::Store(s)         => format!("STORE       {s}"),
        Instruction::Pop              => "POP".to_string(),
        Instruction::Add              => "ADD".to_string(),
        Instruction::Sub              => "SUB".to_string(),
        Instruction::Mul              => "MUL".to_string(),
        Instruction::Div              => "DIV".to_string(),
        Instruction::Neg              => "NEG".to_string(),
        Instruction::Eq               => "EQ".to_string(),
        Instruction::NotEq            => "NOT_EQ".to_string(),
        Instruction::Lt               => "LT".to_string(),
        Instruction::LtEq             => "LT_EQ".to_string(),
        Instruction::Gt               => "GT".to_string(),
        Instruction::GtEq             => "GT_EQ".to_string(),
        Instruction::And              => "AND".to_string(),
        Instruction::Or               => "OR".to_string(),
        Instruction::Not              => "NOT".to_string(),
        Instruction::CallNative(id,n) => format!("CALL_NATIVE {id} {n}"),
        Instruction::Jump(t)          => format!("JUMP        {t}"),
        Instruction::JumpIfFalse(t)   => format!("JUMP_IF_FALSE {t}"),
        Instruction::Return           => "RETURN".to_string(),
        Instruction::Halt             => "HALT".to_string(),
    }
}

fn instr_comment(instr: &Instruction, chunk: &Chunk) -> String {
    match instr {
        Instruction::PushStr(i) => {
            chunk.string_table.get(*i)
                .map(|s| format!("\"{}\"", s.escape_default()))
                .unwrap_or_default()
        }
        Instruction::Load(s) => {
            chunk.local_names.get(*s).cloned().unwrap_or_default()
        }
        Instruction::Store(s) => {
            chunk.local_names.get(*s).cloned().unwrap_or_default()
        }
        Instruction::CallNative(id, _) => {
            chunk.native_table.get(*id).cloned().unwrap_or_default()
        }
        _ => String::new(),
    }
}
