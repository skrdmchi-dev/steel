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
    body: &[Stmt],
    syms: &BTreeMap<String, InferredType>,
) -> Vec<&'static str> {
    let mut types = Vec::with_capacity(params.len());
    for (p, def) in params {
        if let Some(d) = def {
            let t = infer_quick_expr(d, syms);
            if t != InferredType::Any && t != InferredType::Null {
                types.push(ts_type(&t));
                continue;
            }
        }
        if is_param_numeric_only(p, body) {
            types.push("number");
        } else {
            types.push("any");
        }
    }
    types
}

fn is_param_numeric_only(param: &str, body: &[Stmt]) -> bool {
    let mut used_num = false;
    let mut used_other = false;
    fn scan_expr(e: &Expr, p: &str, num: &mut bool, other: &mut bool) {
        match e {
            Expr::Binary { left, op, right } => {
                let is_num_op = matches!(
                    op,
                    BinaryOp::Sub
                        | BinaryOp::Mul
                        | BinaryOp::Div
                        | BinaryOp::Mod
                        | BinaryOp::Pow
                        | BinaryOp::Lt
                        | BinaryOp::Gt
                        | BinaryOp::Lte
                        | BinaryOp::Gte
                );
                if is_num_op {
                    if matches!(&**left, Expr::Ident(n) if n == p)
                        || matches!(&**right, Expr::Ident(n) if n == p)
                    {
                        *num = true;
                    }
                } else if matches!(op, BinaryOp::Add) {
                    if (matches!(&**left, Expr::Ident(n) if n == p)
                        && matches!(&**right, Expr::String(_)))
                        || (matches!(&**right, Expr::Ident(n) if n == p)
                            && matches!(&**left, Expr::String(_)))
                    {
                        *other = true;
                    }
                }
                scan_expr(left, p, num, other);
                scan_expr(right, p, num, other);
            }
            Expr::Member { target, .. } | Expr::Index { target, .. } => {
                if matches!(&**target, Expr::Ident(n) if n == p) {
                    *other = true;
                }
                scan_expr(target, p, num, other);
            }
            Expr::Call { callee, args } => {
                if matches!(&**callee, Expr::Ident(n) if n == p) {
                    *other = true;
                }
                for a in args {
                    scan_expr(a, p, num, other);
                }
            }
            Expr::Unary { expr, .. } | Expr::Spread(expr) => scan_expr(expr, p, num, other),
            Expr::Ternary {
                cond,
                then_expr,
                else_expr,
            } => {
                scan_expr(cond, p, num, other);
                scan_expr(then_expr, p, num, other);
                scan_expr(else_expr, p, num, other);
            }
            Expr::Array(xs) => {
                for x in xs {
                    scan_expr(x, p, num, other);
                }
            }
            Expr::Map(es) => {
                for (_, v) in es {
                    scan_expr(v, p, num, other);
                }
            }
            _ => {}
        }
    }
    fn scan_stmts(stmts: &[Stmt], p: &str, num: &mut bool, other: &mut bool) {
        for s in stmts {
            match s {
                Stmt::Let { value, .. }
                | Stmt::Expr(value)
                | Stmt::Throw(value)
                | Stmt::Assign { value, .. } => scan_expr(value, p, num, other),
                Stmt::Return(Some(e)) => scan_expr(e, p, num, other),
                Stmt::If {
                    cond,
                    then_branch,
                    else_branch,
                } => {
                    scan_expr(cond, p, num, other);
                    scan_stmts(then_branch, p, num, other);
                    if let Some(eb) = else_branch {
                        scan_stmts(eb, p, num, other);
                    }
                }
                Stmt::While { cond, body } => {
                    scan_expr(cond, p, num, other);
                    scan_stmts(body, p, num, other);
                }
                Stmt::For { iterable, body, .. } => {
                    scan_expr(iterable, p, num, other);
                    scan_stmts(body, p, num, other);
                }
                Stmt::TryCatch {
                    try_body,
                    catch_body,
                    ..
                } => {
                    scan_stmts(try_body, p, num, other);
                    scan_stmts(catch_body, p, num, other);
                }
                _ => {}
            }
        }
    }
    scan_stmts(body, param, &mut used_num, &mut used_other);
    used_num && !used_other
}

fn infer_return_type(
    body: &[Stmt],
    local_syms: &BTreeMap<String, InferredType>,
) -> &'static str {
    let mut ret_types: Vec<InferredType> = Vec::new();
    fn collect(stmts: &[Stmt], syms: &BTreeMap<String, InferredType>, out: &mut Vec<InferredType>) {
        for s in stmts {
            match s {
                Stmt::Return(Some(e)) => out.push(infer_quick_expr(e, syms)),
                Stmt::Return(None) => out.push(InferredType::Null),
                Stmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    collect(then_branch, syms, out);
                    if let Some(eb) = else_branch {
                        collect(eb, syms, out);
                    }
                }
                Stmt::While { body, .. } | Stmt::For { body, .. } => collect(body, syms, out),
                Stmt::TryCatch {
                    try_body,
                    catch_body,
                    ..
                } => {
                    collect(try_body, syms, out);
                    collect(catch_body, syms, out);
                }
                _ => {}
            }
        }
    }
    collect(body, local_syms, &mut ret_types);
    if ret_types.is_empty() {
        return "void";
    }
    let first = &ret_types[0];
    if *first == InferredType::Any || *first == InferredType::Null {
        return "any";
    }
    if ret_types.iter().all(|t| t == first) {
        ts_type(first)
    } else {
        "any"
    }
}
