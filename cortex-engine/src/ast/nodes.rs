use serde::Serialize;
use crate::lexer::Span;
use super::types::TypeAnn;

// ── Top-level ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct Program {
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub name: Ident,
    pub params: Vec<Param>,
    pub return_ty: TypeAnn,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct Param {
    pub name: Ident,
    pub ty: TypeAnn,
    pub span: Span,
}

// ── Statements ─────────────────────────────────────────────────────────────

pub type Block = Vec<Stmt>;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "stmt")]
pub enum Stmt {
    Let(LetStmt),
    If(IfStmt),
    Return(ReturnStmt),
    Expr(ExprStmt),
}

#[derive(Debug, Clone, Serialize)]
pub struct LetStmt {
    pub name: Ident,
    pub ty: TypeAnn,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct IfStmt {
    pub condition: Expr,
    pub then_block: Block,
    pub else_branch: Option<ElseBranch>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum ElseBranch {
    Block(Block),
    ElseIf(Box<IfStmt>),
}

#[derive(Debug, Clone, Serialize)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExprStmt {
    pub expr: Expr,
    pub span: Span,
}

// ── Expressions ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "expr")]
pub enum Expr {
    Literal(Literal),
    Ident(Ident),
    Binary(Box<BinaryExpr>),
    Unary(Box<UnaryExpr>),
    Call(Box<CallExpr>),
    NativeCall(Box<NativeCallExpr>),
    Index(Box<IndexExpr>),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal(l) => l.span,
            Expr::Ident(i) => i.span,
            Expr::Binary(b) => b.span,
            Expr::Unary(u) => u.span,
            Expr::Call(c) => c.span,
            Expr::NativeCall(n) => n.span,
            Expr::Index(i) => i.span,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BinaryExpr {
    pub op: BinOp,
    pub left: Expr,
    pub right: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum BinOp {
    Add, Sub, Mul, Div,
    Eq, NotEq,
    Lt, LtEq, Gt, GtEq,
    And, Or,
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*", BinOp::Div => "/",
            BinOp::Eq => "==", BinOp::NotEq => "!=",
            BinOp::Lt => "<", BinOp::LtEq => "<=", BinOp::Gt => ">", BinOp::GtEq => ">=",
            BinOp::And => "&&", BinOp::Or => "||",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub operand: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallExpr {
    pub callee: Ident,
    pub args: Vec<Expr>,
    pub span: Span,
}

/// native.namespace.function(args)
#[derive(Debug, Clone, Serialize)]
pub struct NativeCallExpr {
    /// e.g. ["fs", "read"] for native.fs.read(...)
    pub path: Vec<String>,
    pub args: Vec<Expr>,
    pub span: Span,
}

impl NativeCallExpr {
    /// Returns "native.fs.read" style string for error messages and permission checks.
    pub fn qualified_name(&self) -> String {
        format!("native.{}", self.path.join("."))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexExpr {
    pub object: Expr,
    pub index: Expr,
    pub span: Span,
}

// ── Literals ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct Literal {
    pub kind: LitKind,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum LitKind {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}

// ── Identifier ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}
