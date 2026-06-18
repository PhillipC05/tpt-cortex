use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("error[R001]: execution budget exhausted after {ops} ops — infinite loop or too-expensive task")]
    Timeout { ops: u64 },

    #[error("error[R002]: stack underflow — VM bug")]
    StackUnderflow,

    #[error("error[R003]: type error at runtime — expected {expected}, got {found}")]
    TypeError { expected: &'static str, found: &'static str },

    #[error("error[R004]: unknown native API '{api}'")]
    UnknownNative { api: String },

    #[error("error[R005]: native call failed for '{api}': {msg}")]
    NativeFailed { api: String, msg: String },

    #[error("error[R006]: division by zero")]
    DivisionByZero,
}
