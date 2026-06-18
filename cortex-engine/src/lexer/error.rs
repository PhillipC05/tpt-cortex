use thiserror::Error;
use super::Span;

#[derive(Debug, Error)]
pub enum LexError {
    #[error("error[L001]: unexpected character '{ch}' ({span})")]
    UnexpectedChar { ch: char, span: Span },

    #[error("error[L002]: unterminated string literal ({span})")]
    UnterminatedString { span: Span },

    #[error("error[L003]: invalid escape sequence '\\{ch}' ({span})")]
    InvalidEscape { ch: char, span: Span },

    #[error("error[L004]: invalid number literal '{lit}' ({span})")]
    InvalidNumber { lit: String, span: Span },
}
