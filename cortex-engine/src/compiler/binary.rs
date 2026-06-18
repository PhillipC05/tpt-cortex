/// Binary bytecode encoder/decoder for Cortex `.ctxb` files.
///
/// Format (little-endian throughout):
///
/// ```text
/// [4]  magic   = b"CTXB"
/// [1]  version = 1
/// [4]  chunk count (u32)
/// for each chunk:
///   [4+n] task_name (u32 length + UTF-8 bytes)
///   [4]   local_count (u32)
///   [4]   local_names count (u32)
///   ...   local_names (each: u32 len + UTF-8)
///   [4]   string_table count (u32)
///   ...   strings (each: u32 len + UTF-8)
///   [4]   native_table count (u32)
///   ...   natives (each: u32 len + UTF-8)
///   [4]   instruction count (u32)
///   ...   instructions (opcode byte + typed operands)
/// ```
///
/// Instruction opcodes:
/// ```text
/// 0x00 PushI32  [4: i32]
/// 0x01 PushF64  [8: f64]
/// 0x02 PushStr  [4: u32 index]
/// 0x03 PushBool [1: u8]
/// 0x04 PushVoid
/// 0x05 Load     [4: u32 slot]
/// 0x06 Store    [4: u32 slot]
/// 0x07 Pop
/// 0x08 Add  0x09 Sub  0x0a Mul  0x0b Div  0x0c Neg
/// 0x0d Eq   0x0e NotEq 0x0f Lt  0x10 LtEq 0x11 Gt  0x12 GtEq
/// 0x13 And  0x14 Or   0x15 Not
/// 0x16 CallNative [4: u32 api_id] [4: u32 argc]
/// 0x17 Jump        [4: u32 target]
/// 0x18 JumpIfFalse [4: u32 target]
/// 0x19 Return
/// 0x1a Halt
/// ```
use std::io::{self, Read, Write};

use super::chunk::{Chunk, Instruction};

const MAGIC: &[u8; 4] = b"CTXB";
const VERSION: u8 = 1;

// ── Encode ───────────────────────────────────────────────────────────────────

/// Encode a list of compiled chunks into the `.ctxb` binary format.
pub fn encode_chunks(chunks: &[Chunk]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(MAGIC);
    buf.push(VERSION);
    write_u32(&mut buf, chunks.len() as u32);
    for chunk in chunks {
        encode_chunk(&mut buf, chunk);
    }
    buf
}

fn encode_chunk(buf: &mut Vec<u8>, chunk: &Chunk) {
    write_str(buf, &chunk.task_name);
    write_u32(buf, chunk.local_count as u32);
    write_str_table(buf, &chunk.local_names);
    write_str_table(buf, &chunk.string_table);
    write_str_table(buf, &chunk.native_table);
    write_u32(buf, chunk.instructions.len() as u32);
    for instr in &chunk.instructions {
        encode_instruction(buf, instr);
    }
}

fn encode_instruction(buf: &mut Vec<u8>, instr: &Instruction) {
    match instr {
        Instruction::PushI32(n)  => { buf.push(0x00); buf.extend_from_slice(&n.to_le_bytes()); }
        Instruction::PushF64(f)  => { buf.push(0x01); buf.extend_from_slice(&f.to_le_bytes()); }
        Instruction::PushStr(i)  => { buf.push(0x02); write_u32(buf, *i as u32); }
        Instruction::PushBool(b) => { buf.push(0x03); buf.push(*b as u8); }
        Instruction::PushVoid    => { buf.push(0x04); }
        Instruction::Load(s)     => { buf.push(0x05); write_u32(buf, *s as u32); }
        Instruction::Store(s)    => { buf.push(0x06); write_u32(buf, *s as u32); }
        Instruction::Pop         => { buf.push(0x07); }
        Instruction::Add         => { buf.push(0x08); }
        Instruction::Sub         => { buf.push(0x09); }
        Instruction::Mul         => { buf.push(0x0a); }
        Instruction::Div         => { buf.push(0x0b); }
        Instruction::Neg         => { buf.push(0x0c); }
        Instruction::Eq          => { buf.push(0x0d); }
        Instruction::NotEq       => { buf.push(0x0e); }
        Instruction::Lt          => { buf.push(0x0f); }
        Instruction::LtEq        => { buf.push(0x10); }
        Instruction::Gt          => { buf.push(0x11); }
        Instruction::GtEq        => { buf.push(0x12); }
        Instruction::And         => { buf.push(0x13); }
        Instruction::Or          => { buf.push(0x14); }
        Instruction::Not         => { buf.push(0x15); }
        Instruction::CallNative(api_id, argc) => {
            buf.push(0x16);
            write_u32(buf, *api_id as u32);
            write_u32(buf, *argc as u32);
        }
        Instruction::Jump(t)        => { buf.push(0x17); write_u32(buf, *t as u32); }
        Instruction::JumpIfFalse(t) => { buf.push(0x18); write_u32(buf, *t as u32); }
        Instruction::Return         => { buf.push(0x19); }
        Instruction::Halt           => { buf.push(0x1a); }
    }
}

fn write_u32(buf: &mut Vec<u8>, n: u32) {
    buf.extend_from_slice(&n.to_le_bytes());
}

fn write_str(buf: &mut Vec<u8>, s: &str) {
    write_u32(buf, s.len() as u32);
    buf.extend_from_slice(s.as_bytes());
}

fn write_str_table(buf: &mut Vec<u8>, table: &[String]) {
    write_u32(buf, table.len() as u32);
    for s in table {
        write_str(buf, s);
    }
}

// ── Decode ───────────────────────────────────────────────────────────────────

/// Decode a `.ctxb` binary blob into a list of chunks.
pub fn decode_chunks(data: &[u8]) -> Result<Vec<Chunk>, String> {
    let mut r = Reader { data, pos: 0 };
    let magic = r.read_bytes(4).map_err(|e| format!("read magic: {e}"))?;
    if magic != MAGIC {
        return Err(format!("invalid magic: expected CTXB, got {:?}", &magic[..]));
    }
    let version = r.read_u8().map_err(|e| format!("read version: {e}"))?;
    if version != VERSION {
        return Err(format!("unsupported .ctxb version {version} (expected {VERSION})"));
    }
    let count = r.read_u32().map_err(|e| format!("read chunk count: {e}"))? as usize;
    let mut chunks = Vec::with_capacity(count);
    for i in 0..count {
        chunks.push(decode_chunk(&mut r).map_err(|e| format!("chunk {i}: {e}"))?);
    }
    Ok(chunks)
}

fn decode_chunk(r: &mut Reader) -> Result<Chunk, String> {
    let task_name   = r.read_str()?;
    let local_count = r.read_u32()? as usize;
    let local_names = r.read_str_table()?;
    let string_table = r.read_str_table()?;
    let native_table = r.read_str_table()?;
    let instr_count  = r.read_u32()? as usize;

    let mut instructions = Vec::with_capacity(instr_count);
    for _ in 0..instr_count {
        instructions.push(decode_instruction(r)?);
    }

    Ok(Chunk {
        task_name,
        local_count,
        local_names,
        string_table,
        native_table,
        instructions,
    })
}

fn decode_instruction(r: &mut Reader) -> Result<Instruction, String> {
    let op = r.read_u8()?;
    Ok(match op {
        0x00 => Instruction::PushI32(r.read_i32()?),
        0x01 => Instruction::PushF64(r.read_f64()?),
        0x02 => Instruction::PushStr(r.read_u32()? as usize),
        0x03 => Instruction::PushBool(r.read_u8()? != 0),
        0x04 => Instruction::PushVoid,
        0x05 => Instruction::Load(r.read_u32()? as usize),
        0x06 => Instruction::Store(r.read_u32()? as usize),
        0x07 => Instruction::Pop,
        0x08 => Instruction::Add,
        0x09 => Instruction::Sub,
        0x0a => Instruction::Mul,
        0x0b => Instruction::Div,
        0x0c => Instruction::Neg,
        0x0d => Instruction::Eq,
        0x0e => Instruction::NotEq,
        0x0f => Instruction::Lt,
        0x10 => Instruction::LtEq,
        0x11 => Instruction::Gt,
        0x12 => Instruction::GtEq,
        0x13 => Instruction::And,
        0x14 => Instruction::Or,
        0x15 => Instruction::Not,
        0x16 => Instruction::CallNative(r.read_u32()? as usize, r.read_u32()? as usize),
        0x17 => Instruction::Jump(r.read_u32()? as usize),
        0x18 => Instruction::JumpIfFalse(r.read_u32()? as usize),
        0x19 => Instruction::Return,
        0x1a => Instruction::Halt,
        other => return Err(format!("unknown opcode 0x{:02x}", other)),
    })
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.pos + n > self.data.len() {
            return Err(format!("unexpected end of file at byte {}", self.pos));
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        Ok(self.read_bytes(1)?[0])
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_i32(&mut self) -> Result<i32, String> {
        let b = self.read_bytes(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_f64(&mut self) -> Result<f64, String> {
        let b = self.read_bytes(8)?;
        Ok(f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }

    fn read_str(&mut self) -> Result<String, String> {
        let len = self.read_u32()? as usize;
        let bytes = self.read_bytes(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|e| format!("invalid UTF-8: {e}"))
    }

    fn read_str_table(&mut self) -> Result<Vec<String>, String> {
        let count = self.read_u32()? as usize;
        (0..count).map(|_| self.read_str()).collect()
    }
}

// Suppress unused import warnings from std::io (kept for symmetry with Write trait)
#[allow(dead_code)]
fn _check_io_traits() {
    fn _w(_: &mut dyn Write) {}
    fn _r(_: &mut dyn Read) {}
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compile, checker::PermissionManifest, compile_to_chunks};

    fn chunks_for(src: &str) -> Vec<Chunk> {
        let manifest = PermissionManifest::allow_all();
        let ast = compile(src, &manifest).expect("compile failed");
        compile_to_chunks(&ast)
    }

    #[test]
    fn test_roundtrip_simple() {
        let chunks = chunks_for("task add(a: i32, b: i32) -> i32 { return a + b; }");
        let encoded = encode_chunks(&chunks);
        assert!(encoded.starts_with(b"CTXB"), "missing magic");
        let decoded = decode_chunks(&encoded).expect("decode failed");
        assert_eq!(decoded.len(), chunks.len());
        assert_eq!(decoded[0].task_name, chunks[0].task_name);
        assert_eq!(decoded[0].instructions.len(), chunks[0].instructions.len());
        assert_eq!(decoded[0].local_count, chunks[0].local_count);
    }

    #[test]
    fn test_roundtrip_with_strings() {
        let chunks = chunks_for(r#"task greet() -> void { native.log("hello"); }"#);
        let encoded = encode_chunks(&chunks);
        let decoded = decode_chunks(&encoded).expect("decode failed");
        assert!(decoded[0].string_table.contains(&"hello".to_string()));
        assert!(decoded[0].native_table.contains(&"native.log".to_string()));
    }

    #[test]
    fn test_roundtrip_f64() {
        let chunks = chunks_for("task pi() -> f64 { return 3.14; }");
        let encoded = encode_chunks(&chunks);
        let decoded = decode_chunks(&encoded).expect("decode failed");
        // Verify there's a PushF64 instruction with the right value
        let has_f64 = decoded[0].instructions.iter().any(|i| matches!(i, Instruction::PushF64(f) if (*f - 3.14).abs() < 1e-9));
        assert!(has_f64, "expected PushF64(3.14) instruction");
    }

    #[test]
    fn test_bad_magic() {
        let err = decode_chunks(b"BADD\x01\x00\x00\x00\x00").unwrap_err();
        assert!(err.contains("invalid magic"), "got: {err}");
    }

    #[test]
    fn test_bad_version() {
        let mut data = b"CTXB".to_vec();
        data.push(99); // bad version
        data.extend_from_slice(&0u32.to_le_bytes());
        let err = decode_chunks(&data).unwrap_err();
        assert!(err.contains("unsupported"), "got: {err}");
    }
}
