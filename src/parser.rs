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
