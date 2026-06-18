use super::{LexError, Span, Token};

/// A token paired with its source location.
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub token: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(token: T, span: Span) -> Self {
        Self { token, span }
    }
}

pub fn tokenize(source: &str) -> Result<Vec<Spanned<Token>>, LexError> {
    Lexer::new(source).tokenize()
}

struct Lexer<'src> {
    src: &'src str,
    chars: std::iter::Peekable<std::str::CharIndices<'src>>,
    line: u32,
    col: u32,
    tokens: Vec<Spanned<Token>>,
}

impl<'src> Lexer<'src> {
    fn new(src: &'src str) -> Self {
        Self {
            src,
            chars: src.char_indices().peekable(),
            line: 1,
            col: 1,
            tokens: Vec::new(),
        }
    }

    fn tokenize(mut self) -> Result<Vec<Spanned<Token>>, LexError> {
        loop {
            self.skip_whitespace_and_comments();

            let (start_byte, ch) = match self.chars.peek().copied() {
                None => break,
                Some(pair) => pair,
            };

            let start_col = self.col;
            let start_line = self.line;

            macro_rules! span_of {
                ($len:expr) => {
                    Span::new(start_line, start_col, $len)
                };
            }

            match ch {
                // ── Single-char tokens ─────────────────────
                '(' => { self.advance(); self.push(Token::LParen, span_of!(1)); }
                ')' => { self.advance(); self.push(Token::RParen, span_of!(1)); }
                '{' => { self.advance(); self.push(Token::LBrace, span_of!(1)); }
                '}' => { self.advance(); self.push(Token::RBrace, span_of!(1)); }
                '[' => { self.advance(); self.push(Token::LBracket, span_of!(1)); }
                ']' => { self.advance(); self.push(Token::RBracket, span_of!(1)); }
                ',' => { self.advance(); self.push(Token::Comma, span_of!(1)); }
                ':' => { self.advance(); self.push(Token::Colon, span_of!(1)); }
                ';' => { self.advance(); self.push(Token::Semicolon, span_of!(1)); }
                '.' => { self.advance(); self.push(Token::Dot, span_of!(1)); }
                '*' => { self.advance(); self.push(Token::Star, span_of!(1)); }

                // ── Two-char or single-char ────────────────
                '+' => { self.advance(); self.push(Token::Plus, span_of!(1)); }

                '-' => {
                    self.advance();
                    if self.peek_char() == Some('>') {
                        self.advance();
                        self.push(Token::Arrow, span_of!(2));
                    } else {
                        self.push(Token::Minus, span_of!(1));
                    }
                }

                '!' => {
                    self.advance();
                    if self.peek_char() == Some('=') {
                        self.advance();
                        self.push(Token::NotEq, span_of!(2));
                    } else {
                        self.push(Token::Bang, span_of!(1));
                    }
                }

                '=' => {
                    self.advance();
                    if self.peek_char() == Some('=') {
                        self.advance();
                        self.push(Token::Eq, span_of!(2));
                    } else {
                        self.push(Token::Assign, span_of!(1));
                    }
                }

                '<' => {
                    self.advance();
                    if self.peek_char() == Some('=') {
                        self.advance();
                        self.push(Token::LtEq, span_of!(2));
                    } else {
                        self.push(Token::Lt, span_of!(1));
                    }
                }

                '>' => {
                    self.advance();
                    if self.peek_char() == Some('=') {
                        self.advance();
                        self.push(Token::GtEq, span_of!(2));
                    } else {
                        self.push(Token::Gt, span_of!(1));
                    }
                }

                '&' => {
                    self.advance();
                    if self.peek_char() == Some('&') {
                        self.advance();
                        self.push(Token::And, span_of!(2));
                    } else {
                        return Err(LexError::UnexpectedChar { ch: '&', span: span_of!(1) });
                    }
                }

                '|' => {
                    self.advance();
                    if self.peek_char() == Some('|') {
                        self.advance();
                        self.push(Token::Or, span_of!(2));
                    } else {
                        return Err(LexError::UnexpectedChar { ch: '|', span: span_of!(1) });
                    }
                }

                '/' => {
                    // Comments are handled in skip_whitespace_and_comments; here it's division.
                    self.advance();
                    self.push(Token::Slash, span_of!(1));
                }

                // ── String literals ────────────────────────
                '"' => {
                    let tok = self.lex_string(start_line, start_col)?;
                    self.tokens.push(tok);
                }

                // ── Numbers ───────────────────────────────
                '0'..='9' => {
                    let tok = self.lex_number(start_byte, start_line, start_col)?;
                    self.tokens.push(tok);
                }

                // ── Identifiers & keywords ────────────────
                c if c.is_alphabetic() || c == '_' => {
                    let tok = self.lex_ident(start_byte, start_line, start_col);
                    self.tokens.push(tok);
                }

                other => {
                    return Err(LexError::UnexpectedChar {
                        ch: other,
                        span: span_of!(1),
                    });
                }
            }
        }

        let eof_span = Span::new(self.line, self.col, 0);
        self.push(Token::Eof, eof_span);
        Ok(self.tokens)
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    fn advance(&mut self) -> Option<char> {
        match self.chars.next() {
            Some((_, ch)) => {
                if ch == '\n' {
                    self.line += 1;
                    self.col = 1;
                } else {
                    self.col += 1;
                }
                Some(ch)
            }
            None => None,
        }
    }

    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().map(|&(_, c)| c)
    }

    fn push(&mut self, token: Token, span: Span) {
        self.tokens.push(Spanned::new(token, span));
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek_char() {
                Some(' ') | Some('\t') | Some('\r') | Some('\n') => {
                    self.advance();
                }
                Some('/') => {
                    // Peek one further to check for `//`
                    let mut iter = self.chars.clone();
                    iter.next(); // consume '/'
                    if iter.peek().map(|&(_, c)| c) == Some('/') {
                        // Consume until end of line
                        while self.peek_char().is_some() && self.peek_char() != Some('\n') {
                            self.advance();
                        }
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
    }

    fn lex_string(&mut self, start_line: u32, start_col: u32) -> Result<Spanned<Token>, LexError> {
        self.advance(); // consume opening `"`
        let mut s = String::new();
        loop {
            match self.peek_char() {
                None | Some('\n') => {
                    return Err(LexError::UnterminatedString {
                        span: Span::new(start_line, start_col, 1),
                    });
                }
                Some('"') => {
                    self.advance();
                    break;
                }
                Some('\\') => {
                    self.advance(); // consume `\`
                    let esc_col = self.col;
                    match self.peek_char() {
                        Some('n') => { self.advance(); s.push('\n'); }
                        Some('t') => { self.advance(); s.push('\t'); }
                        Some('r') => { self.advance(); s.push('\r'); }
                        Some('"') => { self.advance(); s.push('"'); }
                        Some('\\') => { self.advance(); s.push('\\'); }
                        Some(other) => {
                            self.advance();
                            return Err(LexError::InvalidEscape {
                                ch: other,
                                span: Span::new(self.line, esc_col, 2),
                            });
                        }
                        None => {
                            return Err(LexError::UnterminatedString {
                                span: Span::new(start_line, start_col, 1),
                            });
                        }
                    }
                }
                Some(c) => {
                    s.push(c);
                    self.advance();
                }
            }
        }
        let len = s.len() as u32 + 2; // include surrounding quotes in span
        Ok(Spanned::new(Token::StringLit(s), Span::new(start_line, start_col, len)))
    }

    fn lex_number(
        &mut self,
        start_byte: usize,
        start_line: u32,
        start_col: u32,
    ) -> Result<Spanned<Token>, LexError> {
        let mut is_float = false;

        while self.peek_char().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            self.advance();
        }

        if self.peek_char() == Some('.') {
            // Check the char after '.' to avoid consuming `..` or `.method`
            let mut lookahead = self.chars.clone();
            lookahead.next(); // skip '.'
            if lookahead.peek().map(|&(_, c)| c.is_ascii_digit()).unwrap_or(false) {
                is_float = true;
                self.advance(); // consume '.'
                while self.peek_char().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    self.advance();
                }
            }
        }

        // Find the end byte by peeking at the iterator
        let end_byte = self.chars.peek().map(|&(i, _)| i).unwrap_or(self.src.len());
        let raw = &self.src[start_byte..end_byte];
        let len = raw.len() as u32;
        let span = Span::new(start_line, start_col, len);

        if is_float {
            raw.parse::<f64>()
                .map(|v| Spanned::new(Token::FloatLit(v), span))
                .map_err(|_| LexError::InvalidNumber { lit: raw.to_string(), span })
        } else {
            raw.parse::<i64>()
                .map(|v| Spanned::new(Token::IntLit(v), span))
                .map_err(|_| LexError::InvalidNumber { lit: raw.to_string(), span })
        }
    }

    fn lex_ident(&mut self, start_byte: usize, start_line: u32, start_col: u32) -> Spanned<Token> {
        while self
            .peek_char()
            .map(|c| c.is_alphanumeric() || c == '_')
            .unwrap_or(false)
        {
            self.advance();
        }

        let end_byte = self.chars.peek().map(|&(i, _)| i).unwrap_or(self.src.len());
        let word = &self.src[start_byte..end_byte];
        let len = word.len() as u32;
        let span = Span::new(start_line, start_col, len);

        let token = match word {
            "task"   => Token::Task,
            "let"    => Token::Let,
            "if"     => Token::If,
            "else"   => Token::Else,
            "return" => Token::Return,
            "true"   => Token::True,
            "false"  => Token::False,
            "i32"    => Token::TyI32,
            "f64"    => Token::TyF64,
            "string" => Token::TyString,
            "bool"   => Token::TyBool,
            "list"   => Token::TyList,
            "map"    => Token::TyMap,
            "void"   => Token::TyVoid,
            "native" => Token::Native,
            _        => Token::Ident(word.to_string()),
        };

        Spanned::new(token, span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Token> {
        tokenize(src).unwrap().into_iter().map(|s| s.token).collect()
    }

    #[test]
    fn keywords() {
        let t = toks("task let if else return true false");
        assert_eq!(t[0], Token::Task);
        assert_eq!(t[1], Token::Let);
        assert_eq!(t[2], Token::If);
        assert_eq!(t[3], Token::Else);
        assert_eq!(t[4], Token::Return);
        assert_eq!(t[5], Token::True);
        assert_eq!(t[6], Token::False);
    }

    #[test]
    fn type_keywords() {
        let t = toks("i32 f64 string bool list map void");
        assert_eq!(t[0], Token::TyI32);
        assert_eq!(t[1], Token::TyF64);
        assert_eq!(t[2], Token::TyString);
        assert_eq!(t[3], Token::TyBool);
        assert_eq!(t[4], Token::TyList);
        assert_eq!(t[5], Token::TyMap);
        assert_eq!(t[6], Token::TyVoid);
    }

    #[test]
    fn integer_literal() {
        let t = toks("42");
        assert_eq!(t[0], Token::IntLit(42));
    }

    #[test]
    fn float_literal() {
        let t = toks("3.14");
        assert_eq!(t[0], Token::FloatLit(3.14));
    }

    #[test]
    fn string_literal() {
        let t = toks(r#""hello world""#);
        assert_eq!(t[0], Token::StringLit("hello world".into()));
    }

    #[test]
    fn string_escape_sequences() {
        let t = toks(r#""line1\nline2""#);
        assert_eq!(t[0], Token::StringLit("line1\nline2".into()));
    }

    #[test]
    fn native_prefix() {
        let t = toks("native.log");
        assert_eq!(t[0], Token::Native);
        assert_eq!(t[1], Token::Dot);
        assert_eq!(t[2], Token::Ident("log".into()));
    }

    #[test]
    fn arrow() {
        let t = toks("->");
        assert_eq!(t[0], Token::Arrow);
    }

    #[test]
    fn two_char_ops() {
        let t = toks("== != <= >= && ||");
        assert_eq!(t[0], Token::Eq);
        assert_eq!(t[1], Token::NotEq);
        assert_eq!(t[2], Token::LtEq);
        assert_eq!(t[3], Token::GtEq);
        assert_eq!(t[4], Token::And);
        assert_eq!(t[5], Token::Or);
    }

    #[test]
    fn spans_track_line_and_col() {
        let tokens = tokenize("task\nhello").unwrap();
        assert_eq!(tokens[0].span.line, 1);
        assert_eq!(tokens[0].span.col, 1);
        assert_eq!(tokens[1].span.line, 2);
        assert_eq!(tokens[1].span.col, 1);
    }

    #[test]
    fn comments_are_skipped() {
        let t = toks("// this is a comment\ntask");
        assert_eq!(t[0], Token::Task);
    }

    #[test]
    fn unterminated_string_errors() {
        assert!(tokenize(r#""oops"#).is_err());
    }

    #[test]
    fn invalid_escape_errors() {
        assert!(tokenize(r#""\q""#).is_err());
    }
}
