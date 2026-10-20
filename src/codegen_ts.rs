use crate::ast::{AssignTarget, BinaryOp, ClassMethod, Expr, Program, Stmt, UnaryOp};
use crate::codegen::ModuleUnit;
use crate::typecheck::{analyze_program, InferredType};
use std::collections::{BTreeMap, HashSet};

fn indent(lvl: usize) -> String {
    "    ".repeat(lvl)
}

fn escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

fn ts_type(ty: &InferredType) -> &'static str {
    match ty {
        InferredType::Num => "number",
        InferredType::Str => "string",
        InferredType::Bool => "boolean",
        InferredType::Null => "null",
        InferredType::Array => "any[]",
        InferredType::Map => "Record<string, any>",
        InferredType::Func { .. } => "(...args: any[]) => any",
        InferredType::Class(_) | InferredType::Any => "any",
    }
}

fn infer_quick_expr(expr: &Expr, syms: &BTreeMap<String, InferredType>) -> InferredType {
    match expr {
        Expr::Number(_) => InferredType::Num,
        Expr::String(_) => InferredType::Str,
        Expr::Bool(_) => InferredType::Bool,
        Expr::Null => InferredType::Null,
        Expr::Array(_) | Expr::Range { .. } => InferredType::Array,
        Expr::Map(_) => InferredType::Map,
        Expr::Ident(n) => syms.get(n).cloned().unwrap_or(InferredType::Any),
        Expr::Unary { op: UnaryOp::Not, .. } => InferredType::Bool,
        Expr::Unary { .. } => InferredType::Num,
        Expr::Binary { left, op, right } => match op {
            BinaryOp::Eq
            | BinaryOp::Neq
            | BinaryOp::Lt
            | BinaryOp::Gt
