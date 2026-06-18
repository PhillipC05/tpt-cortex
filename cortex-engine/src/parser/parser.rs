use crate::ast::*;
use crate::lexer::{Span, Spanned, Token};
use super::ParseError;

pub fn parse(tokens: Vec<Spanned<Token>>) -> Result<Program, Vec<ParseError>> {
    Parser::new(tokens).parse_program()
}

struct Parser {
    tokens: Vec<Spanned<Token>>,
    pos: usize,
    errors: Vec<ParseError>,
}

impl Parser {
    fn new(tokens: Vec<Spanned<Token>>) -> Self {
        Self { tokens, pos: 0, errors: Vec::new() }
    }

    // ── Cursor helpers ────────────────────────────────────────────────────

    fn peek(&self) -> &Spanned<Token> {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn peek_token(&self) -> &Token {
        &self.peek().token
    }

    fn span(&self) -> Span {
        self.peek().span
    }

    fn advance(&mut self) -> &Spanned<Token> {
        let t = &self.tokens[self.pos.min(self.tokens.len() - 1)];
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek_token(), Token::Eof)
    }

    fn check(&self, tok: &Token) -> bool {
        std::mem::discriminant(self.peek_token()) == std::mem::discriminant(tok)
    }

    fn eat(&mut self, tok: &Token) -> Result<Spanned<Token>, ParseError> {
        if std::mem::discriminant(self.peek_token()) == std::mem::discriminant(tok) {
            Ok(self.advance().clone())
        } else {
            Err(ParseError::Unexpected {
                found: self.peek_token().clone(),
                expected: tok.to_string(),
                span: self.span(),
            })
        }
    }

    fn eat_ident(&mut self) -> Result<Ident, ParseError> {
        let sp = self.span();
        match self.peek_token().clone() {
            Token::Ident(name) => {
                self.advance();
                Ok(Ident { name, span: sp })
            }
            other => Err(ParseError::Unexpected {
                found: other,
                expected: "identifier".to_string(),
                span: sp,
            }),
        }
    }

    // ── Top-level ─────────────────────────────────────────────────────────

    fn parse_program(&mut self) -> Result<Program, Vec<ParseError>> {
        let mut tasks = Vec::new();
        while !self.at_eof() {
            match self.parse_task() {
                Ok(t) => tasks.push(t),
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }
        if self.errors.is_empty() {
            Ok(Program { tasks })
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    /// Skip tokens until we find the start of a new declaration (for error recovery).
    fn synchronize(&mut self) {
        while !self.at_eof() {
            if matches!(self.peek_token(), Token::Task) {
                break;
            }
            self.advance();
        }
    }

    // ── Task declaration ──────────────────────────────────────────────────

    fn parse_task(&mut self) -> Result<Task, ParseError> {
        let start = self.span();
        self.eat(&Token::Task)?;
        let name = self.eat_ident()?;
        self.eat(&Token::LParen)?;
        let params = self.parse_param_list()?;
        self.eat(&Token::RParen)?;
        self.eat(&Token::Arrow)?;
        let return_ty = self.parse_type()?;
        self.eat(&Token::LBrace)?;
        let body = self.parse_block()?;
        let end = self.span();
        self.eat(&Token::RBrace)?;
        Ok(Task {
            name,
            params,
            return_ty,
            body,
            span: Span::new(start.line, start.col, end.col - start.col + 1),
        })
    }

    fn parse_param_list(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        if self.check(&Token::RParen) {
            return Ok(params);
        }
        params.push(self.parse_param()?);
        while self.check(&Token::Comma) {
            self.advance();
            if self.check(&Token::RParen) { break; }
            params.push(self.parse_param()?);
        }
        Ok(params)
    }

    fn parse_param(&mut self) -> Result<Param, ParseError> {
        let start = self.span();
        let name = self.eat_ident()?;
        self.eat(&Token::Colon)?;
        let ty = self.parse_type()?;
        Ok(Param { name, span: start, ty })
    }

    // ── Types ─────────────────────────────────────────────────────────────

    fn parse_type(&mut self) -> Result<TypeAnn, ParseError> {
        let span = self.span();
        let kind = match self.peek_token().clone() {
            Token::TyI32   => { self.advance(); TypeKind::I32 }
            Token::TyF64   => { self.advance(); TypeKind::F64 }
            Token::TyString => { self.advance(); TypeKind::Str }
            Token::TyBool  => { self.advance(); TypeKind::Bool }
            Token::TyVoid  => { self.advance(); TypeKind::Void }
            Token::TyList  => {
                self.advance();
                self.eat(&Token::Lt)?;
                let inner = self.parse_type()?;
                self.eat(&Token::Gt)?;
                TypeKind::List(Box::new(inner))
            }
            Token::TyMap   => {
                self.advance();
                self.eat(&Token::Lt)?;
                let key = self.parse_type()?;
                self.eat(&Token::Comma)?;
                let val = self.parse_type()?;
                self.eat(&Token::Gt)?;
                TypeKind::Map(Box::new(key), Box::new(val))
            }
            other => return Err(ParseError::Unexpected {
                found: other,
                expected: "type (i32, f64, string, bool, list<T>, map<K,V>, void)".to_string(),
                span,
            }),
        };
        Ok(TypeAnn { kind, span })
    }

    // ── Statements ────────────────────────────────────────────────────────

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        let mut stmts = Vec::new();
        while !self.check(&Token::RBrace) && !self.at_eof() {
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        match self.peek_token() {
            Token::Let    => self.parse_let().map(Stmt::Let),
            Token::If     => self.parse_if().map(Stmt::If),
            Token::Return => self.parse_return().map(Stmt::Return),
            _             => self.parse_expr_stmt().map(Stmt::Expr),
        }
    }

    fn parse_let(&mut self) -> Result<LetStmt, ParseError> {
        let start = self.span();
        self.eat(&Token::Let)?;
        let name = self.eat_ident()?;
        self.eat(&Token::Colon)?;
        let ty = self.parse_type()?;
        self.eat(&Token::Assign)?;
        let value = self.parse_expr()?;
        self.eat(&Token::Semicolon)?;
        Ok(LetStmt { name, ty, value, span: start })
    }

    fn parse_if(&mut self) -> Result<IfStmt, ParseError> {
        let start = self.span();
        self.eat(&Token::If)?;
        let condition = self.parse_expr()?;
        self.eat(&Token::LBrace)?;
        let then_block = self.parse_block()?;
        self.eat(&Token::RBrace)?;

        let else_branch = if self.check(&Token::Else) {
            self.advance();
            if self.check(&Token::If) {
                Some(ElseBranch::ElseIf(Box::new(self.parse_if()?)))
            } else {
                self.eat(&Token::LBrace)?;
                let blk = self.parse_block()?;
                self.eat(&Token::RBrace)?;
                Some(ElseBranch::Block(blk))
            }
        } else {
            None
        };

        Ok(IfStmt { condition, then_block, else_branch, span: start })
    }

    fn parse_return(&mut self) -> Result<ReturnStmt, ParseError> {
        let start = self.span();
        self.eat(&Token::Return)?;
        let value = if !self.check(&Token::Semicolon) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.eat(&Token::Semicolon)?;
        Ok(ReturnStmt { value, span: start })
    }

    fn parse_expr_stmt(&mut self) -> Result<ExprStmt, ParseError> {
        let start = self.span();
        let expr = self.parse_expr()?;
        self.eat(&Token::Semicolon)?;
        Ok(ExprStmt { expr, span: start })
    }

    // ── Expressions (Pratt) ───────────────────────────────────────────────

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_pratt(0)
    }

    fn parse_pratt(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_unary()?;

        loop {
            let op = match self.peek_token() {
                Token::Or    => BinOp::Or,
                Token::And   => BinOp::And,
                Token::Eq    => BinOp::Eq,
                Token::NotEq => BinOp::NotEq,
                Token::Lt    => BinOp::Lt,
                Token::LtEq  => BinOp::LtEq,
                Token::Gt    => BinOp::Gt,
                Token::GtEq  => BinOp::GtEq,
                Token::Plus  => BinOp::Add,
                Token::Minus => BinOp::Sub,
                Token::Star  => BinOp::Mul,
                Token::Slash => BinOp::Div,
                _ => break,
            };

            let (l_bp, r_bp) = infix_bp(&op);
            if l_bp < min_bp { break; }

            let op_span = self.span();
            self.advance();
            let rhs = self.parse_pratt(r_bp)?;
            let span = Span::new(lhs.span().line, lhs.span().col, rhs.span().col - lhs.span().col + 1);
            lhs = Expr::Binary(Box::new(BinaryExpr { op, left: lhs, right: rhs, span }));
            let _ = op_span;
        }

        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        let span = self.span();
        match self.peek_token().clone() {
            Token::Bang => {
                self.advance();
                let operand = self.parse_unary()?;
                let end = operand.span();
                Ok(Expr::Unary(Box::new(UnaryExpr {
                    op: UnaryOp::Not,
                    span: Span::new(span.line, span.col, end.col - span.col + 1),
                    operand,
                })))
            }
            Token::Minus => {
                self.advance();
                let operand = self.parse_unary()?;
                let end = operand.span();
                Ok(Expr::Unary(Box::new(UnaryExpr {
                    op: UnaryOp::Neg,
                    span: Span::new(span.line, span.col, end.col - span.col + 1),
                    operand,
                })))
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.check(&Token::LBracket) {
                let start = self.span();
                self.advance();
                let index = self.parse_expr()?;
                let end = self.span();
                self.eat(&Token::RBracket)?;
                expr = Expr::Index(Box::new(IndexExpr {
                    object: expr,
                    index,
                    span: Span::new(start.line, start.col, end.col - start.col + 1),
                }));
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let span = self.span();
        match self.peek_token().clone() {
            Token::IntLit(n) => {
                self.advance();
                Ok(Expr::Literal(Literal { kind: LitKind::Int(n), span }))
            }
            Token::FloatLit(f) => {
                self.advance();
                Ok(Expr::Literal(Literal { kind: LitKind::Float(f), span }))
            }
            Token::StringLit(s) => {
                self.advance();
                Ok(Expr::Literal(Literal { kind: LitKind::Str(s), span }))
            }
            Token::True => {
                self.advance();
                Ok(Expr::Literal(Literal { kind: LitKind::Bool(true), span }))
            }
            Token::False => {
                self.advance();
                Ok(Expr::Literal(Literal { kind: LitKind::Bool(false), span }))
            }
            Token::Native => {
                self.parse_native_call()
            }
            Token::Ident(name) => {
                self.advance();
                let ident = Ident { name, span };
                if self.check(&Token::LParen) {
                    // Function call
                    self.advance();
                    let args = self.parse_arg_list()?;
                    let end = self.span();
                    self.eat(&Token::RParen)?;
                    Ok(Expr::Call(Box::new(CallExpr {
                        callee: ident,
                        args,
                        span: Span::new(span.line, span.col, end.col - span.col + 1),
                    })))
                } else {
                    Ok(Expr::Ident(ident))
                }
            }
            Token::LParen => {
                self.advance();
                let inner = self.parse_expr()?;
                self.eat(&Token::RParen)?;
                Ok(inner)
            }
            other => Err(ParseError::Unexpected {
                found: other,
                expected: "expression".to_string(),
                span,
            }),
        }
    }

    /// Parse `native.namespace.fn(args)`.
    fn parse_native_call(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();
        self.eat(&Token::Native)?;  // consume `native`

        let mut path = Vec::new();
        // Consume `.ident` segments
        while self.check(&Token::Dot) {
            self.advance(); // consume `.`
            let seg = self.eat_ident()?;
            path.push(seg.name);
        }

        if path.is_empty() {
            return Err(ParseError::Custom {
                message: "native call requires at least one path segment after 'native.' (e.g. native.log or native.fs.read)".to_string(),
                span: start,
            });
        }

        self.eat(&Token::LParen)?;
        let args = self.parse_arg_list()?;
        let end = self.span();
        self.eat(&Token::RParen)?;

        Ok(Expr::NativeCall(Box::new(NativeCallExpr {
            path,
            args,
            span: Span::new(start.line, start.col, end.col - start.col + 1),
        })))
    }

    fn parse_arg_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if self.check(&Token::RParen) {
            return Ok(args);
        }
        args.push(self.parse_expr()?);
        while self.check(&Token::Comma) {
            self.advance();
            if self.check(&Token::RParen) { break; }
            args.push(self.parse_expr()?);
        }
        Ok(args)
    }
}

// ── Pratt operator binding powers ────────────────────────────────────────────
// Higher number = tighter binding.
// Returns (left_bp, right_bp). right_bp > left_bp means left-associative.

fn infix_bp(op: &BinOp) -> (u8, u8) {
    match op {
        BinOp::Or             => (1, 2),
        BinOp::And            => (3, 4),
        BinOp::Eq | BinOp::NotEq => (5, 6),
        BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq => (7, 8),
        BinOp::Add | BinOp::Sub => (9, 10),
        BinOp::Mul | BinOp::Div => (11, 12),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    fn parse_src(src: &str) -> Program {
        let tokens = tokenize(src).expect("lex failed");
        parse(tokens).expect("parse failed")
    }

    #[test]
    fn empty_task() {
        let p = parse_src("task noop() -> void {}");
        assert_eq!(p.tasks.len(), 1);
        assert_eq!(p.tasks[0].name.name, "noop");
        assert!(p.tasks[0].body.is_empty());
    }

    #[test]
    fn task_with_params() {
        let p = parse_src("task add(a: i32, b: i32) -> i32 { return a + b; }");
        let t = &p.tasks[0];
        assert_eq!(t.params.len(), 2);
        assert_eq!(t.params[0].name.name, "a");
        assert_eq!(t.params[1].name.name, "b");
    }

    #[test]
    fn let_statement() {
        let p = parse_src(r#"task t() -> void { let x: i32 = 42; }"#);
        assert!(matches!(p.tasks[0].body[0], Stmt::Let(_)));
    }

    #[test]
    fn if_else() {
        let p = parse_src("task t() -> void { if true { let x: i32 = 1; } else { let y: i32 = 2; } }");
        assert!(matches!(p.tasks[0].body[0], Stmt::If(_)));
    }

    #[test]
    fn native_call_two_segments() {
        let p = parse_src(r#"task t() -> void { native.log("hi"); }"#);
        let stmt = &p.tasks[0].body[0];
        if let Stmt::Expr(e) = stmt {
            if let Expr::NativeCall(nc) = &e.expr {
                assert_eq!(nc.path, vec!["log"]);
                return;
            }
        }
        panic!("expected native call");
    }

    #[test]
    fn native_call_three_segments() {
        let p = parse_src(r#"task t() -> void { native.fs.read("/tmp/x"); }"#);
        let stmt = &p.tasks[0].body[0];
        if let Stmt::Expr(e) = stmt {
            if let Expr::NativeCall(nc) = &e.expr {
                assert_eq!(nc.path, vec!["fs", "read"]);
                return;
            }
        }
        panic!("expected native call");
    }

    #[test]
    fn operator_precedence() {
        // 2 + 3 * 4 should parse as 2 + (3 * 4)
        let p = parse_src("task t() -> void { let x: i32 = 2 + 3 * 4; }");
        if let Stmt::Let(l) = &p.tasks[0].body[0] {
            if let Expr::Binary(b) = &l.value {
                assert_eq!(b.op, BinOp::Add);
                if let Expr::Binary(rhs) = &b.right {
                    assert_eq!(rhs.op, BinOp::Mul);
                    return;
                }
            }
        }
        panic!("precedence wrong");
    }

    #[test]
    fn list_type() {
        let p = parse_src("task t() -> void { let xs: list<string> = native.db.query(\"q\"); }");
        if let Stmt::Let(l) = &p.tasks[0].body[0] {
            assert!(matches!(l.ty.kind, TypeKind::List(_)));
        }
    }

    #[test]
    fn returns_error_on_bad_syntax() {
        let tokens = tokenize("task bad( -> void {}").unwrap();
        assert!(parse(tokens).is_err());
    }
}
