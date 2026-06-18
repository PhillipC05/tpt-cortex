use crate::ast::*;
use super::{CheckError, PermissionManifest, scope::ScopeStack};

pub fn check(program: &Program, manifest: &PermissionManifest) -> Result<(), Vec<CheckError>> {
    let mut checker = Checker::new(manifest);
    checker.check_program(program);
    if checker.errors.is_empty() {
        Ok(())
    } else {
        Err(checker.errors)
    }
}

struct Checker<'m> {
    manifest: &'m PermissionManifest,
    scopes: ScopeStack,
    errors: Vec<CheckError>,
    /// Return type of the current task being checked.
    current_return_ty: Option<TypeKind>,
}

impl<'m> Checker<'m> {
    fn new(manifest: &'m PermissionManifest) -> Self {
        Self {
            manifest,
            scopes: ScopeStack::new(),
            errors: Vec::new(),
            current_return_ty: None,
        }
    }

    fn error(&mut self, e: CheckError) {
        self.errors.push(e);
    }

    // ── Program / Task ────────────────────────────────────────────────────

    fn check_program(&mut self, program: &Program) {
        for task in &program.tasks {
            self.check_task(task);
        }
    }

    fn check_task(&mut self, task: &Task) {
        self.scopes.push();
        self.current_return_ty = Some(task.return_ty.kind.clone());

        for param in &task.params {
            if !self.scopes.declare(&param.name.name, param.ty.kind.clone()) {
                self.error(CheckError::AlreadyDeclared {
                    name: param.name.name.clone(),
                    span: param.span,
                });
            }
        }

        self.check_block(&task.body);
        self.scopes.pop();
        self.current_return_ty = None;
    }

    // ── Statements ────────────────────────────────────────────────────────

    fn check_block(&mut self, block: &[Stmt]) {
        self.scopes.push();
        for stmt in block {
            self.check_stmt(stmt);
        }
        self.scopes.pop();
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(s)    => self.check_let(s),
            Stmt::If(s)     => self.check_if(s),
            Stmt::Return(s) => self.check_return(s),
            Stmt::Expr(s)   => { self.infer_expr(&s.expr); }
        }
    }

    fn check_let(&mut self, stmt: &LetStmt) {
        let val_ty = self.infer_expr(&stmt.value);
        if let Some(ty) = val_ty {
            if !types_compatible(&stmt.ty.kind, &ty) {
                self.error(CheckError::TypeMismatch {
                    expected: stmt.ty.kind.to_string(),
                    found: ty.to_string(),
                    span: stmt.value.span(),
                });
            }
        }
        if !self.scopes.declare(&stmt.name.name, stmt.ty.kind.clone()) {
            self.error(CheckError::AlreadyDeclared {
                name: stmt.name.name.clone(),
                span: stmt.span,
            });
        }
    }

    fn check_if(&mut self, stmt: &IfStmt) {
        let cond_ty = self.infer_expr(&stmt.condition);
        if let Some(ty) = cond_ty {
            if ty != TypeKind::Bool {
                self.error(CheckError::TypeMismatch {
                    expected: "bool".to_string(),
                    found: ty.to_string(),
                    span: stmt.condition.span(),
                });
            }
        }
        self.check_block(&stmt.then_block);
        if let Some(else_branch) = &stmt.else_branch {
            match else_branch {
                ElseBranch::Block(blk) => self.check_block(blk),
                ElseBranch::ElseIf(s)  => self.check_if(s),
            }
        }
    }

    fn check_return(&mut self, stmt: &ReturnStmt) {
        if let Some(val) = &stmt.value {
            let ret_ty = self.infer_expr(val);
            if let (Some(ret_ty), Some(expected)) = (ret_ty, &self.current_return_ty.clone()) {
                if !types_compatible(expected, &ret_ty) {
                    self.error(CheckError::TypeMismatch {
                        expected: expected.to_string(),
                        found: ret_ty.to_string(),
                        span: val.span(),
                    });
                }
            }
        }
    }

    // ── Expressions ───────────────────────────────────────────────────────
    // Returns the inferred type, or None if inference failed (error already recorded).

    fn infer_expr(&mut self, expr: &Expr) -> Option<TypeKind> {
        match expr {
            Expr::Literal(l) => Some(match &l.kind {
                LitKind::Int(_)   => TypeKind::I32,
                LitKind::Float(_) => TypeKind::F64,
                LitKind::Str(_)   => TypeKind::Str,
                LitKind::Bool(_)  => TypeKind::Bool,
            }),

            Expr::Ident(id) => {
                match self.scopes.lookup(&id.name) {
                    Some(b) => Some(b.ty.clone()),
                    None => {
                        self.error(CheckError::Undefined {
                            name: id.name.clone(),
                            span: id.span,
                        });
                        None
                    }
                }
            }

            Expr::Binary(b) => self.infer_binary(b),

            Expr::Unary(u) => {
                let operand_ty = self.infer_expr(&u.operand)?;
                match u.op {
                    UnaryOp::Not => {
                        if operand_ty != TypeKind::Bool {
                            self.error(CheckError::TypeMismatch {
                                expected: "bool".to_string(),
                                found: operand_ty.to_string(),
                                span: u.operand.span(),
                            });
                        }
                        Some(TypeKind::Bool)
                    }
                    UnaryOp::Neg => {
                        if !is_numeric(&operand_ty) {
                            self.error(CheckError::TypeMismatch {
                                expected: "i32 or f64".to_string(),
                                found: operand_ty.to_string(),
                                span: u.operand.span(),
                            });
                        }
                        Some(operand_ty)
                    }
                }
            }

            Expr::Call(c) => {
                // Regular function calls — we don't have user-defined functions yet,
                // so just check the args and return void for now.
                for arg in &c.args { self.infer_expr(arg); }
                Some(TypeKind::Void)
            }

            Expr::NativeCall(nc) => {
                let qualified = nc.qualified_name();
                if !self.manifest.is_allowed(&qualified) {
                    self.error(CheckError::PermissionDenied {
                        api: qualified,
                        span: nc.span,
                    });
                }
                for arg in &nc.args { self.infer_expr(arg); }
                // Native calls return void at this stage; Phase 2 will add typed returns.
                Some(TypeKind::Void)
            }

            Expr::Index(i) => {
                self.infer_expr(&i.object);
                let idx_ty = self.infer_expr(&i.index)?;
                if idx_ty != TypeKind::I32 {
                    self.error(CheckError::TypeMismatch {
                        expected: "i32".to_string(),
                        found: idx_ty.to_string(),
                        span: i.index.span(),
                    });
                }
                // Return type unknown without generics resolution — return void for now.
                Some(TypeKind::Void)
            }
        }
    }

    fn infer_binary(&mut self, b: &BinaryExpr) -> Option<TypeKind> {
        let lhs_ty = self.infer_expr(&b.left)?;
        let rhs_ty = self.infer_expr(&b.right)?;

        match &b.op {
            // Arithmetic: both sides must be same numeric type
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                if is_numeric(&lhs_ty) && types_compatible(&lhs_ty, &rhs_ty) {
                    Some(lhs_ty)
                } else if b.op == BinOp::Add && lhs_ty == TypeKind::Str && rhs_ty == TypeKind::Str {
                    // String concatenation via +
                    Some(TypeKind::Str)
                } else {
                    self.error(CheckError::TypeMismatch {
                        expected: lhs_ty.to_string(),
                        found: rhs_ty.to_string(),
                        span: b.right.span(),
                    });
                    None
                }
            }

            // Comparison: both sides must be same type, result is bool
            BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq => {
                if !types_compatible(&lhs_ty, &rhs_ty) {
                    self.error(CheckError::TypeMismatch {
                        expected: lhs_ty.to_string(),
                        found: rhs_ty.to_string(),
                        span: b.right.span(),
                    });
                }
                Some(TypeKind::Bool)
            }

            // Equality: both sides must be same type, result is bool
            BinOp::Eq | BinOp::NotEq => {
                if !types_compatible(&lhs_ty, &rhs_ty) {
                    self.error(CheckError::TypeMismatch {
                        expected: lhs_ty.to_string(),
                        found: rhs_ty.to_string(),
                        span: b.right.span(),
                    });
                }
                Some(TypeKind::Bool)
            }

            // Logical: both sides must be bool
            BinOp::And | BinOp::Or => {
                if lhs_ty != TypeKind::Bool {
                    self.error(CheckError::TypeMismatch {
                        expected: "bool".to_string(),
                        found: lhs_ty.to_string(),
                        span: b.left.span(),
                    });
                }
                if rhs_ty != TypeKind::Bool {
                    self.error(CheckError::TypeMismatch {
                        expected: "bool".to_string(),
                        found: rhs_ty.to_string(),
                        span: b.right.span(),
                    });
                }
                Some(TypeKind::Bool)
            }
        }
    }
}

// ── Type helpers ──────────────────────────────────────────────────────────────

fn types_compatible(expected: &TypeKind, found: &TypeKind) -> bool {
    // Void is assignable from native calls (until typed native returns land in Phase 2).
    if matches!(expected, TypeKind::Void) || matches!(found, TypeKind::Void) {
        return true;
    }
    expected == found
}

fn is_numeric(ty: &TypeKind) -> bool {
    matches!(ty, TypeKind::I32 | TypeKind::F64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn check_src(src: &str, allow: &[&str]) -> Result<(), Vec<CheckError>> {
        let manifest = PermissionManifest::new(allow.iter().map(|s| s.to_string()));
        let tokens = tokenize(src).unwrap();
        let ast = parse(tokens).unwrap();
        check(&ast, &manifest)
    }

    #[test]
    fn valid_program_passes() {
        assert!(check_src(
            r#"task t() -> void { let x: i32 = 42; }"#,
            &[]
        ).is_ok());
    }

    #[test]
    fn type_mismatch_caught() {
        let r = check_src(r#"task t() -> void { let x: i32 = "hello"; }"#, &[]);
        assert!(r.is_err());
        let errs = r.unwrap_err();
        assert!(matches!(errs[0], CheckError::TypeMismatch { .. }));
    }

    #[test]
    fn undefined_variable_caught() {
        let r = check_src(r#"task t() -> void { let x: i32 = y; }"#, &[]);
        assert!(r.is_err());
    }

    #[test]
    fn permission_denied_without_allow() {
        let r = check_src(
            r#"task t() -> void { native.fs.write("/tmp/x", "hi"); }"#,
            &["native.log"], // fs.write not in manifest
        );
        assert!(r.is_err());
        let errs = r.unwrap_err();
        assert!(matches!(errs[0], CheckError::PermissionDenied { .. }));
    }

    #[test]
    fn permission_allowed_passes() {
        assert!(check_src(
            r#"task t() -> void { native.log("hi"); }"#,
            &["native.log"]
        ).is_ok());
    }

    #[test]
    fn empty_manifest_allows_all() {
        // When --allow is not specified, everything is permitted.
        assert!(check_src(
            r#"task t() -> void { native.fs.write("/tmp/x", "hi"); }"#,
            &[]
        ).is_ok());
    }

    #[test]
    fn string_concat_allowed() {
        assert!(check_src(
            r#"task t() -> void { let s: string = "hello" + " world"; }"#,
            &[]
        ).is_ok());
    }

    #[test]
    fn if_condition_must_be_bool() {
        let r = check_src(r#"task t() -> void { if 42 { let x: i32 = 1; } }"#, &[]);
        assert!(r.is_err());
    }
}
