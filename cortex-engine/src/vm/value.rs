/// A runtime value on the Cortex VM stack.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    I32(i32),
    F64(f64),
    Str(String),
    Bool(bool),
    Void,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::I32(n)  => write!(f, "{}", n),
            Value::F64(n)  => write!(f, "{}", n),
            Value::Str(s)  => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Void    => write!(f, "()"),
        }
    }
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::I32(_)  => "i32",
            Value::F64(_)  => "f64",
            Value::Str(_)  => "string",
            Value::Bool(_) => "bool",
            Value::Void    => "void",
        }
    }
}
