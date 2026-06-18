use serde::Serialize;
use crate::lexer::Span;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TypeAnn {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "inner")]
pub enum TypeKind {
    I32,
    F64,
    Str,
    Bool,
    Void,
    List(Box<TypeAnn>),
    Map(Box<TypeAnn>, Box<TypeAnn>),
}

impl std::fmt::Display for TypeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeKind::I32 => write!(f, "i32"),
            TypeKind::F64 => write!(f, "f64"),
            TypeKind::Str => write!(f, "string"),
            TypeKind::Bool => write!(f, "bool"),
            TypeKind::Void => write!(f, "void"),
            TypeKind::List(t) => write!(f, "list<{}>", t.kind),
            TypeKind::Map(k, v) => write!(f, "map<{}, {}>", k.kind, v.kind),
        }
    }
}
