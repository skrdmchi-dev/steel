#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Number(f64),
    String(String),
    Ident(String),
    True,
    False,
    Null,

    // Keywords
    Let,
    Const,
    Fn,
    Return,
    If,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    Match,
    Class,
    Extends,
    New,
    Try,
    Catch,
    Throw,
    Import,
    Test,
    Enum,
    Defer,

    // Operators
    Plus,          // +
    Minus,         // -
