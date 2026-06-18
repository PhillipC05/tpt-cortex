use std::collections::HashMap;
use crate::ast::TypeKind;

/// A variable binding in the symbol table.
#[derive(Debug, Clone)]
pub struct Binding {
    pub ty: TypeKind,
}

/// Lexically-scoped symbol table.
pub struct ScopeStack {
    scopes: Vec<HashMap<String, Binding>>,
}

impl ScopeStack {
    pub fn new() -> Self {
        Self { scopes: vec![HashMap::new()] }
    }

    pub fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop(&mut self) {
        self.scopes.pop();
    }

    pub fn declare(&mut self, name: &str, ty: TypeKind) -> bool {
        let top = self.scopes.last_mut().unwrap();
        if top.contains_key(name) {
            return false; // already declared in this scope
        }
        top.insert(name.to_string(), Binding { ty });
        true
    }

    pub fn lookup(&self, name: &str) -> Option<&Binding> {
        for scope in self.scopes.iter().rev() {
            if let Some(b) = scope.get(name) {
                return Some(b);
            }
        }
        None
    }
}
