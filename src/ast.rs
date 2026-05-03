#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Pow,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    NullCoalesce,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    String(String),
    Bool(bool),
    Null,
    Ident(String),
    Array(Vec<Expr>),
    Map(Vec<(String, Expr)>),
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Ternary {
        cond: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    New {
        class_name: String,
        args: Vec<Expr>,
    },
    Index {
        target: Box<Expr>,
        index: Box<Expr>,
    },
    Member {
        target: Box<Expr>,
        field: String,
    },
    FnExpr {
        params: Vec<(String, Option<Expr>)>,
        body: Vec<Stmt>,
    },
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
    },
    Spread(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignTarget {
    Ident(String),
    Index { target: Expr, index: Expr },
    Member { target: Expr, field: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassMethod {
    pub name: String,
    pub params: Vec<(String, Option<Expr>)>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Import(String),
    Let {
        name: String,
        is_const: bool,
        value: Expr,
    },
    Assign {
        target: AssignTarget,
        value: Expr,
    },
    FnDecl {
        name: String,
        params: Vec<(String, Option<Expr>)>,
        body: Vec<Stmt>,
    },
    ClassDecl {
        name: String,
        parent: Option<String>,
        methods: Vec<ClassMethod>,
    },
    TryCatch {
        try_body: Vec<Stmt>,
        err_var: String,
        catch_body: Vec<Stmt>,
    },
    Throw(Expr),
    Test {
        name: String,
