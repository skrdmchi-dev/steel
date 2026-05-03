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
    ThinArrow,     // ->
    DotDotDot,     // ...
    Star,          // *
    StarStar,      // **
    Slash,         // /
    Percent,       // %
    Assign,        // =
    PlusAssign,    // +=
    MinusAssign,   // -=
    StarAssign,    // *=
    SlashAssign,   // /=
    PercentAssign, // %=
    Eq,            // ==
    Neq,           // !=
    Lt,            // <
    Gt,            // >
    Lte,           // <=
    Gte,           // >=
    And,           // &&
    Or,            // ||
    Not,           // !
    BitAnd,        // &
    BitOr,         // |
    BitXor,        // ^
    BitNot,        // ~
    Shl,           // <<
    Shr,           // >>
    Pipe,          // |>
    Arrow,         // =>
    Question,      // ?
    NullCoalesce,  // ??
    DotDot,        // ..
    Dot,           // .

    // Delimiters
    LParen,    // (
    RParen,    // )
    LBrace,    // {
