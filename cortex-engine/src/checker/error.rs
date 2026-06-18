use thiserror::Error;
use crate::lexer::Span;

#[derive(Debug, Error)]
pub enum CheckError {
    #[error("error[E001]: type mismatch — expected {expected}, found {found} ({span})")]
    TypeMismatch { expected: String, found: String, span: Span },

    #[error("error[E002]: permission denied — '{api}' is not in the allowed manifest ({span})")]
    PermissionDenied { api: String, span: Span },

    #[error("error[E003]: undefined variable '{name}' ({span})")]
    Undefined { name: String, span: Span },

    #[error("error[E004]: variable '{name}' already declared in this scope ({span})")]
    AlreadyDeclared { name: String, span: Span },

    #[error("error[E005]: {message} ({span})")]
    Custom { message: String, span: Span },
}
