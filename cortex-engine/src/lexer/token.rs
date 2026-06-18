use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Token {
    // ── Keywords ─────────────────────────────
    Task,
    Let,
    If,
    Else,
    Return,
    True,
    False,

    // ── Type keywords ─────────────────────────
    TyI32,
    TyF64,
    TyString,
    TyBool,
    TyList,
    TyMap,
    TyVoid,

    // ── Literals ──────────────────────────────
    IntLit(i64),
    FloatLit(f64),
    StringLit(String),

    // ── Identifier ────────────────────────────
    Ident(String),

    // ── Native namespace marker ───────────────
    // Represents the `native` keyword; the `.` after it is consumed by the parser.
    Native,

    // ── Operators ─────────────────────────────
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Bang,       // !
    Eq,         // ==
    NotEq,      // !=
    Lt,         // <
    LtEq,       // <=
    Gt,         // >
    GtEq,       // >=
    And,        // &&
    Or,         // ||
    Assign,     // =
    Arrow,      // ->

    // ── Delimiters ────────────────────────────
    LParen,     // (
    RParen,     // )
    LBrace,     // {
    RBrace,     // }
    LBracket,   // [
    RBracket,   // ]
    Comma,      // ,
    Colon,      // :
    Semicolon,  // ;
    Dot,        // .

    // ── End of file ───────────────────────────
    Eof,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Task => write!(f, "task"),
            Token::Let => write!(f, "let"),
            Token::If => write!(f, "if"),
            Token::Else => write!(f, "else"),
            Token::Return => write!(f, "return"),
            Token::True => write!(f, "true"),
            Token::False => write!(f, "false"),
            Token::TyI32 => write!(f, "i32"),
            Token::TyF64 => write!(f, "f64"),
            Token::TyString => write!(f, "string"),
            Token::TyBool => write!(f, "bool"),
            Token::TyList => write!(f, "list"),
            Token::TyMap => write!(f, "map"),
            Token::TyVoid => write!(f, "void"),
            Token::Native => write!(f, "native"),
            Token::IntLit(n) => write!(f, "{}", n),
            Token::FloatLit(n) => write!(f, "{}", n),
            Token::StringLit(s) => write!(f, "\"{}\"", s),
            Token::Ident(s) => write!(f, "{}", s),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Bang => write!(f, "!"),
            Token::Eq => write!(f, "=="),
            Token::NotEq => write!(f, "!="),
            Token::Lt => write!(f, "<"),
            Token::LtEq => write!(f, "<="),
            Token::Gt => write!(f, ">"),
            Token::GtEq => write!(f, ">="),
            Token::And => write!(f, "&&"),
            Token::Or => write!(f, "||"),
            Token::Assign => write!(f, "="),
            Token::Arrow => write!(f, "->"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::Comma => write!(f, ","),
            Token::Colon => write!(f, ":"),
            Token::Semicolon => write!(f, ";"),
            Token::Dot => write!(f, "."),
            Token::Eof => write!(f, "<eof>"),
        }
    }
}
