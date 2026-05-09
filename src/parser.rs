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
