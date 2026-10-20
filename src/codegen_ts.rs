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
            | BinaryOp::Lte
            | BinaryOp::Gte
            | BinaryOp::And
            | BinaryOp::Or => InferredType::Bool,
            BinaryOp::Add => {
                let lt = infer_quick_expr(left, syms);
                let rt = infer_quick_expr(right, syms);
                if lt == InferredType::Str || rt == InferredType::Str {
                    InferredType::Str
                } else if lt == InferredType::Num && rt == InferredType::Num {
                    InferredType::Num
                } else {
                    InferredType::Any
                }
            }
            BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Mod
            | BinaryOp::Pow
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr => InferredType::Num,
            BinaryOp::NullCoalesce => InferredType::Any,
        },
        Expr::Call { callee, .. } => {
            if let Expr::Ident(name) = &**callee {
                match name.as_str() {
                    "len" | "num" | "int" | "abs" | "sqrt" | "floor" | "ceil" | "round" | "sin" | "cos"
                    | "tan" | "atan2" | "log" | "exp" | "pow" | "min" | "max" | "clamp" | "sign"
                    | "sum" | "avg" | "index_of" | "char_code" | "time_ms" | "clock" => {
                        return InferredType::Num;
                    }
                    "str" | "upper" | "lower" | "trim" | "join" | "replace" | "read_file"
                    | "repeat" | "pad_start" | "pad_end" | "from_char_code" | "json_stringify"
                    | "hash" | "type" => return InferredType::Str,
                    "bool" | "contains" | "has_key" | "delete_key" | "starts_with" | "ends_with"
                    | "any" | "all" | "file_exists" | "write_file" | "append_file" | "remove_file"
                    | "assert" => return InferredType::Bool,
                    "split" | "keys" | "values" | "range" | "map" | "filter" | "sort"
                    | "concat" | "unique" | "flat" | "zip" | "enumerate" => {
                        return InferredType::Array;
                    }
                    _ => {
                        if let Some(InferredType::Func { ret, .. }) = syms.get(name) {
                            return (**ret).clone();
                        }
                    }
                }
            }
            InferredType::Any
        }
        Expr::New { class_name, .. } => InferredType::Class(class_name.clone()),
        _ => InferredType::Any,
    }
}

fn infer_param_types(
    params: &[(String, Option<Expr>)],
