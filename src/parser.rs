use crate::ast::{AssignTarget, BinaryOp, ClassMethod, Expr, Program, Stmt, UnaryOp};
use crate::lexer::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn current(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .unwrap_or_else(|| self.tokens.last().unwrap())
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.current().kind
    }

    fn peek_next_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.pos + 1).map(|t| &t.kind)
    }

    fn advance(&mut self) -> Token {
        let tok = self.current().clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    fn eat_semicolons(&mut self) {
        while matches!(self.peek_kind(), TokenKind::Semicolon) {
            self.advance();
        }
    }

    fn expect(&mut self, expected: TokenKind) -> Result<Token, String> {
        let cur = self.current();
        if cur.kind == expected {
            Ok(self.advance())
        } else {
            Err(format!(
                "Expected {:?}, found {:?} at {}:{}",
                expected, cur.kind, cur.line, cur.col
            ))
        }
    }

    fn expect_ident(&mut self) -> Result<String, String> {
        let cur = self.current().clone();
        if let TokenKind::Ident(name) = cur.kind {
            self.advance();
            Ok(name)
        } else {
            Err(format!(
                "Expected identifier, found {:?} at {}:{}",
                cur.kind, cur.line, cur.col
            ))
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, String> {
        let mut statements = Vec::new();
        self.eat_semicolons();
        while !matches!(self.peek_kind(), TokenKind::Eof) {
            statements.append(&mut self.parse_stmt_multi()?);
            self.eat_semicolons();
        }
        Ok(Program { statements })
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        self.expect(TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        self.eat_semicolons();
        while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
            stmts.append(&mut self.parse_stmt_multi()?);
            self.eat_semicolons();
        }
        self.expect(TokenKind::RBrace)?;
        Ok(stmts)
    }

    fn parse_stmt_multi(&mut self) -> Result<Vec<Stmt>, String> {
        // Destructuring let: `let [a, b] = expr` or `let {x, y} = expr`
        if matches!(self.peek_kind(), TokenKind::Let | TokenKind::Const)
            && matches!(
                self.peek_next_kind(),
                Some(TokenKind::LBracket | TokenKind::LBrace)
            )
        {
            let is_const = matches!(self.advance().kind, TokenKind::Const);
            let is_array = matches!(self.advance().kind, TokenKind::LBracket);
            let close_tok = if is_array {
                TokenKind::RBracket
            } else {
                TokenKind::RBrace
            };
            let mut names = Vec::new();
            while !matches!(self.peek_kind(), TokenKind::RBracket | TokenKind::RBrace | TokenKind::Eof) {
                names.push(self.expect_ident()?);
                if matches!(self.peek_kind(), TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(close_tok)?;
            self.expect(TokenKind::Assign)?;
            let rhs = self.parse_expr()?;
            self.eat_semicolons();

            let tmp = format!("__destruct_{}", self.pos);
            let mut out = vec![Stmt::Let {
                name: tmp.clone(),
                is_const,
                value: rhs,
            }];
            for (idx, name) in names.into_iter().enumerate() {
                let val = if is_array {
                    Expr::Index {
                        target: Box::new(Expr::Ident(tmp.clone())),
                        index: Box::new(Expr::Number(idx as f64)),
                    }
                } else {
                    Expr::Member {
                        target: Box::new(Expr::Ident(tmp.clone())),
                        field: name.clone(),
                    }
                };
                out.push(Stmt::Let {
                    name,
                    is_const,
                    value: val,
                });
            }
            return Ok(out);
        }

        Ok(vec![self.parse_stmt()?])
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        match self.peek_kind() {
            TokenKind::Import => {
                self.advance();
                let cur = self.current().clone();
                if let TokenKind::String(path) = cur.kind {
                    self.advance();
                    self.eat_semicolons();
                    Ok(Stmt::Import(path))
                } else {
                    Err(format!(
                        "Expected string path after 'import' at {}:{}",
                        cur.line, cur.col
                    ))
                }
            }
            TokenKind::Let | TokenKind::Const => {
                let is_const = matches!(self.advance().kind, TokenKind::Const);
                let name = self.expect_ident()?;
                // Optional type annotation: `let x: num = 5`
                if matches!(self.peek_kind(), TokenKind::Colon) {
                    self.advance();
                    let _type_name = self.expect_ident()?;
                }
                let value = if matches!(self.peek_kind(), TokenKind::Assign) {
                    self.advance();
                    self.parse_expr()?
                } else {
                    Expr::Null
                };
                self.eat_semicolons();
                Ok(Stmt::Let {
                    name,
                    is_const,
                    value,
                })
            }
            TokenKind::Fn => {
                if matches!(self.peek_next_kind(), Some(TokenKind::Ident(_))) {
                    self.advance();
                    let name = self.expect_ident()?;
                    let params = self.parse_param_list()?;
                    // Optional return type annotation: `fn add(a: num, b: num) -> num { ... }`
                    if matches!(self.peek_kind(), TokenKind::ThinArrow | TokenKind::Colon) {
                        self.advance();
                        let _ret_type = self.expect_ident()?;
                    }
                    let body = if matches!(self.peek_kind(), TokenKind::Assign) {
                        self.advance();
                        let expr = self.parse_expr()?;
                        self.eat_semicolons();
                        vec![Stmt::Return(Some(expr))]
                    } else {
                        self.parse_block()?
                    };
                    Ok(Stmt::FnDecl { name, params, body })
                } else {
                    let expr = self.parse_expr()?;
                    self.eat_semicolons();
                    Ok(Stmt::Expr(expr))
                }
            }
            TokenKind::Test => {
                self.advance();
                let name = match &self.current().kind {
                    TokenKind::String(s) | TokenKind::Ident(s) => {
                        let n = s.clone();
                        self.advance();
                        n
                    }
                    _ => "unnamed_test".to_string(),
                };
                let body = self.parse_block()?;
                Ok(Stmt::Test { name, body })
            }
            TokenKind::Enum => {
                self.advance();
                let name = self.expect_ident()?;
                self.expect(TokenKind::LBrace)?;
                let mut entries = Vec::new();
                let mut next_val = 0.0;
                while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
                    let variant = self.expect_ident()?;
                    let val_expr = if matches!(self.peek_kind(), TokenKind::Assign) {
                        self.advance();
                        let e = self.parse_expr()?;
                        if let Expr::Number(n) = e {
                            next_val = n + 1.0;
                        }
                        e
                    } else {
                        let e = Expr::Number(next_val);
                        next_val += 1.0;
                        e
                    };
                    entries.push((variant, val_expr));
                    if matches!(self.peek_kind(), TokenKind::Comma | TokenKind::Semicolon) {
                        self.advance();
                    }
                }
                self.expect(TokenKind::RBrace)?;
                Ok(Stmt::Let {
                    name,
                    is_const: true,
                    value: Expr::Map(entries),
                })
            }
            TokenKind::Defer => {
                self.advance();
                let inner = if matches!(self.peek_kind(), TokenKind::LBrace) {
                    let stmts = self.parse_block()?;
                    Stmt::If {
                        cond: Expr::Bool(true),
                        then_branch: stmts,
                        else_branch: None,
                    }
                } else {
                    self.parse_stmt()?
                };
                Ok(Stmt::Defer(Box::new(inner)))
            }
            TokenKind::Class => self.parse_class_decl(),
            TokenKind::Try => {
                self.advance();
                let try_body = self.parse_block()?;
                self.expect(TokenKind::Catch)?;
                let err_var = if matches!(self.peek_kind(), TokenKind::LParen) {
                    self.advance();
                    let v = self.expect_ident()?;
                    self.expect(TokenKind::RParen)?;
                    v
                } else if matches!(self.peek_kind(), TokenKind::Ident(_)) {
                    self.expect_ident()?
                } else {
                    "err".to_string()
                };
                let catch_body = self.parse_block()?;
                Ok(Stmt::TryCatch {
                    try_body,
                    err_var,
                    catch_body,
                })
            }
            TokenKind::Throw => {
                self.advance();
                let expr = self.parse_expr()?;
                self.eat_semicolons();
                Ok(Stmt::Throw(expr))
            }
            TokenKind::Return => {
                let tok = self.advance();
                if matches!(
                    self.peek_kind(),
                    TokenKind::Semicolon | TokenKind::RBrace | TokenKind::Eof
                ) || self.current().line > tok.line
                {
                    self.eat_semicolons();
                    Ok(Stmt::Return(None))
                } else {
                    let expr = self.parse_expr()?;
                    self.eat_semicolons();
                    Ok(Stmt::Return(Some(expr)))
                }
            }
            TokenKind::If => self.parse_if_stmt(),
            TokenKind::While => {
                self.advance();
                let cond = self.parse_expr()?;
                let body = self.parse_block()?;
                Ok(Stmt::While { cond, body })
            }
            TokenKind::For => {
                self.advance();
                let has_paren = if matches!(self.peek_kind(), TokenKind::LParen) {
                    self.advance();
                    true
                } else {
                    false
                };
                let var = self.expect_ident()?;
                self.expect(TokenKind::In)?;
                let iterable = self.parse_expr()?;
                if has_paren {
                    self.expect(TokenKind::RParen)?;
                }
                let body = self.parse_block()?;
                Ok(Stmt::For {
                    var,
                    iterable,
                    body,
                })
            }
            TokenKind::Break => {
                self.advance();
                self.eat_semicolons();
                Ok(Stmt::Break)
            }
            TokenKind::Continue => {
