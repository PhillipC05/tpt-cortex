use thiserror::Error;
use crate::lexer::{Span, Token};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("error[P001]: unexpected token '{found}', expected {expected} ({span})")]
    Unexpected { found: Token, expected: String, span: Span },

    #[error("error[P002]: unexpected end of input, expected {expected}")]
    UnexpectedEof { expected: String },

    #[error("error[P003]: {message} ({span})")]
    Custom { message: String, span: Span },
}
