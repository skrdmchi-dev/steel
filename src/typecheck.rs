use crate::ast::{AssignTarget, BinaryOp, Expr, Program, Stmt, UnaryOp};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferredType {
    Num,
    Str,
    Bool,
    Null,
    Array,
    Map,
    Func { params: usize, ret: Box<InferredType> },
    Class(String),
    Any,
}

impl fmt::Display for InferredType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Num => write!(f, "num (f64)"),
            Self::Str => write!(f, "str"),
            Self::Bool => write!(f, "bool"),
            Self::Null => write!(f, "null"),
            Self::Array => write!(f, "array<any>"),
            Self::Map => write!(f, "map<str, any>"),
            Self::Func { params, ret } => write!(f, "fn({} args) -> {}", params, ret),
            Self::Class(c) => write!(f, "class<{}>", c),
            Self::Any => write!(f, "any"),
        }
    }
}

pub struct TypeReport {
    pub symbols: BTreeMap<String, InferredType>,
    pub errors: Vec<String>,
}

pub fn analyze_program(program: &Program) -> TypeReport {
    let mut symbols = BTreeMap::new();
    let mut errors = Vec::new();

    // Pre-seed all built-in standard library symbols
    for builtin in [
