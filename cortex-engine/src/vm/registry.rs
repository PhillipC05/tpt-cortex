use super::{RuntimeError, Value};

/// Implementors map qualified native API names to OS-level behaviour.
/// The CLI provides a minimal implementation; the Go daemon provides the full one.
pub trait NativeRegistry {
    fn call(&mut self, api: &str, args: Vec<Value>) -> Result<Value, RuntimeError>;
}

/// Minimal registry used by `cortex run` in Phase 2.
/// Handles only `native.log`; everything else errors.
pub struct CliRegistry;

impl NativeRegistry for CliRegistry {
    fn call(&mut self, api: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        match api {
            "native.log" => {
                let msg = args.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" ");
                println!("{}", msg);
                Ok(Value::Void)
            }
            other => Err(RuntimeError::UnknownNative { api: other.to_string() }),
        }
    }
}
