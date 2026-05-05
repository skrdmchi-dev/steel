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
    RBrace,    // }
    LBracket,  // [
    RBracket,  // ]
    Comma,     // ,
    Colon,     // :
    Semicolon, // ;

    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();

        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
                continue;
            }

            // Single-line comment
            if ch == '/' && self.peek_next() == Some('/') {
                while let Some(c) = self.peek() {
                    if c == '\n' {
                        break;
                    }
                    self.advance();
                }
                continue;
            }
            // Multi-line comment
            if ch == '/' && self.peek_next() == Some('*') {
                self.advance();
                self.advance();
                while let Some(c) = self.peek() {
                    if c == '*' && self.peek_next() == Some('/') {
                        self.advance();
                        self.advance();
                        break;
                    }
                    self.advance();
                }
                continue;
            }

            let line = self.line;
            let col = self.col;

            // Numbers (decimal, hex 0x, binary 0b)
            if ch.is_ascii_digit() {
                if ch == '0' && matches!(self.peek_next(), Some('x' | 'X')) {
                    self.advance();
                    self.advance();
                    let mut hex = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_ascii_hexdigit() || c == '_' {
                            if c != '_' {
                                hex.push(c);
                            }
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    let val = i64::from_str_radix(&hex, 16)
                        .map_err(|_| format!("Invalid hex literal '0x{}' at {}:{}", hex, line, col))?;
                    tokens.push(Token {
                        kind: TokenKind::Number(val as f64),
                        line,
                        col,
                    });
                    continue;
                }
                if ch == '0' && matches!(self.peek_next(), Some('b' | 'B')) {
                    self.advance();
                    self.advance();
                    let mut bin = String::new();
                    while let Some(c) = self.peek() {
                        if c == '0' || c == '1' || c == '_' {
                            if c != '_' {
                                bin.push(c);
                            }
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    let val = i64::from_str_radix(&bin, 2).map_err(|_| {
                        format!("Invalid binary literal '0b{}' at {}:{}", bin, line, col)
                    })?;
                    tokens.push(Token {
                        kind: TokenKind::Number(val as f64),
                        line,
                        col,
                    });
                    continue;
                }

                let mut num_str = String::new();
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() || c == '_' {
                        if c != '_' {
                            num_str.push(c);
                        }
                        self.advance();
                    } else if c == '.'
                        && self.peek_next().map(|n| n.is_ascii_digit()).unwrap_or(false)
                        && !num_str.contains('.')
                    {
                        num_str.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }
                let val: f64 = num_str
                    .parse()
                    .map_err(|_| format!("Invalid number '{}' at {}:{}", num_str, line, col))?;
                tokens.push(Token {
                    kind: TokenKind::Number(val),
                    line,
                    col,
                });
                continue;
            }

            if ch == '"' || ch == '\'' {
                let quote = ch;
                self.advance();
                let mut s = String::new();
                let mut closed = false;
                while let Some(c) = self.advance() {
                    if c == quote {
                        closed = true;
                        break;
                    }
                    if c == '\\' {
                        if let Some(esc) = self.advance() {
                            match esc {
                                'n' => s.push('\n'),
                                't' => s.push('\t'),
                                'r' => s.push('\r'),
                                '\\' => s.push('\\'),
                                '"' => s.push('"'),
