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
        "print", "println", "input", "assert", "type", "str", "num", "int", "bool", "len", "push", "pop",
        "keys", "values", "has_key", "delete_key", "concat", "contains", "index_of", "starts_with", "ends_with",
        "repeat", "pad_start", "pad_end", "char_code", "from_char_code",
        "range", "slice", "reverse", "sort", "map", "filter", "reduce", "find", "any", "all",
        "sum", "avg", "unique", "flat", "enumerate", "zip", "split", "join", "trim",
        "upper", "lower", "replace", "json_parse", "json_stringify", "args", "env", "set_env",
        "cwd", "exec", "exit", "file_exists", "read_file", "write_file", "append_file",
        "list_dir", "make_dir", "mkdir", "remove_dir", "rename", "remove_file", "clock", "sleep", "random", "rand_int", "hash", "sha256", "sha256_file",
        "uuid", "abs", "sqrt", "pow", "floor", "ceil", "round", "min", "max", "clamp", "sin",
        "cos", "tan", "atan2", "log", "exp", "sign", "PI", "E", "this", "self",
    ] {
        symbols.insert(builtin.to_string(), InferredType::Any);
    }

    // Pre-register top-level functions and classes
    for stmt in &program.statements {
        match stmt {
            Stmt::FnDecl { name, params, .. } => {
                symbols.insert(
                    name.clone(),
                    InferredType::Func {
                        params: params.len(),
                        ret: Box::new(InferredType::Any),
                    },
                );
            }
            Stmt::ClassDecl { name, .. } => {
                symbols.insert(name.clone(), InferredType::Class(name.clone()));
            }
            _ => {}
        }
    }

    for stmt in &program.statements {
        analyze_stmt(stmt, &mut symbols, &mut errors);
    }

    // Remove built-ins from the user symbol table report
    for builtin in [
        "print", "println", "input", "assert", "type", "str", "num", "int", "bool", "len", "push", "pop",
        "keys", "values", "has_key", "delete_key", "concat", "contains", "index_of", "starts_with", "ends_with",
        "repeat", "pad_start", "pad_end", "char_code", "from_char_code",
        "range", "slice", "reverse", "sort", "map", "filter", "reduce", "find", "any", "all",
        "sum", "avg", "unique", "flat", "enumerate", "zip", "split", "join", "trim",
        "upper", "lower", "replace", "json_parse", "json_stringify", "args", "env", "set_env",
        "cwd", "exec", "exit", "file_exists", "read_file", "write_file", "append_file",
        "list_dir", "make_dir", "mkdir", "remove_dir", "rename", "remove_file", "clock", "sleep", "random", "rand_int", "hash", "sha256", "sha256_file",
        "uuid", "abs", "sqrt", "pow", "floor", "ceil", "round", "min", "max", "clamp", "sin",
        "cos", "tan", "atan2", "log", "exp", "sign", "PI", "E", "this", "self",
    ] {
        symbols.remove(builtin);
    }

    TypeReport { symbols, errors }
}

fn unify(a: &InferredType, b: &InferredType) -> InferredType {
    if a == b {
        a.clone()
    } else {
        InferredType::Any
    }
}

fn analyze_stmt(
    stmt: &Stmt,
    env: &mut BTreeMap<String, InferredType>,
    errors: &mut Vec<String>,
) {
    match stmt {
        Stmt::Let { name, value, .. } => {
            check_expr_undefined(value, env, errors);
            let t = infer_expr(value, env);
            env.insert(name.clone(), t);
        }
        Stmt::Assign { target, value } => {
            check_expr_undefined(value, env, errors);
            let t = infer_expr(value, env);
            if let AssignTarget::Ident(name) = target {
                if let Some(InferredType::Func { .. }) = env.get(name) {
                    errors.push(format!(
                        "Cannot overwrite function '{}' without 'let' (use `let res = {}(...)`)",
                        name, name
                    ));
                }
                let merged = match env.get(name) {
                    Some(prev) => unify(prev, &t),
                    None => t,
                };
                env.insert(name.clone(), merged);
            }
        }
        Stmt::FnDecl { name, params, body } => {
            let mut local = env.clone();
            for (p, def) in params {
                if let Some(d) = def {
                    check_expr_undefined(d, &local, errors);
                }
                let dt = def
                    .as_ref()
                    .map(|e| infer_expr(e, &local))
                    .unwrap_or(InferredType::Any);
                local.insert(p.clone(), dt);
            }
            let mut ret_type = InferredType::Null;
            let mut saw_return = false;
            for st in body {
                analyze_stmt(st, &mut local, errors);
                if let Some(rt) = find_return_type(st, &local) {
                    ret_type = if !saw_return {
                        saw_return = true;
                        rt
                    } else {
                        unify(&ret_type, &rt)
                    };
                }
            }
            env.insert(
                name.clone(),
                InferredType::Func {
                    params: params.len(),
                    ret: Box::new(ret_type),
                },
            );
        }
        Stmt::ClassDecl { name, methods, .. } => {
            env.insert(name.clone(), InferredType::Class(name.clone()));
            for m in methods {
                let mut local = env.clone();
                for (p, _) in &m.params {
                    local.insert(p.clone(), InferredType::Any);
                }
                for st in &m.body {
                    analyze_stmt(st, &mut local, errors);
                }
            }
        }
        Stmt::For {
            var,
            iterable,
            body,
        } => {
            check_expr_undefined(iterable, env, errors);
            let elem_ty = match iterable {
                Expr::Range { .. } => InferredType::Num,
                _ => InferredType::Any,
            };
            env.insert(var.clone(), elem_ty);
            for st in body {
                analyze_stmt(st, env, errors);
            }
        }
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            check_expr_undefined(cond, env, errors);
            for st in then_branch {
                analyze_stmt(st, env, errors);
            }
            if let Some(eb) = else_branch {
                for st in eb {
                    analyze_stmt(st, env, errors);
                }
            }
        }
        Stmt::While { cond, body } => {
            check_expr_undefined(cond, env, errors);
            for st in body {
                analyze_stmt(st, env, errors);
            }
        }
        Stmt::Test { body, .. } => {
            let mut local = env.clone();
            for st in body {
                analyze_stmt(st, &mut local, errors);
            }
        }
        Stmt::TryCatch {
            try_body,
            err_var,
            catch_body,
        } => {
            for st in try_body {
                analyze_stmt(st, env, errors);
            }
            env.insert(err_var.clone(), InferredType::Str);
            for st in catch_body {
                analyze_stmt(st, env, errors);
            }
        }
        Stmt::Return(Some(e)) | Stmt::Throw(e) | Stmt::Expr(e) => {
            check_expr_undefined(e, env, errors);
        }
        Stmt::Defer(inner) => analyze_stmt(inner, env, errors),
        _ => {}
    }
}

fn check_expr_undefined(
    expr: &Expr,
    env: &BTreeMap<String, InferredType>,
    errors: &mut Vec<String>,
) {
    match expr {
        Expr::Ident(name) => {
            if !env.contains_key(name) {
                errors.push(format!("Undefined variable '{}'", name));
            }
        }
        Expr::Array(items) => {
            for it in items {
                check_expr_undefined(it, env, errors);
            }
        }
        Expr::Map(entries) => {
            for (_, v) in entries {
                check_expr_undefined(v, env, errors);
            }
        }
        Expr::Binary { left, right, .. } | Expr::Range { start: left, end: right } => {
            check_expr_undefined(left, env, errors);
            check_expr_undefined(right, env, errors);
        }
        Expr::Unary { expr: inner, .. } | Expr::Spread(inner) => {
            check_expr_undefined(inner, env, errors);
        }
        Expr::Ternary {
            cond,
            then_expr,
            else_expr,
        } => {
            check_expr_undefined(cond, env, errors);
            check_expr_undefined(then_expr, env, errors);
            check_expr_undefined(else_expr, env, errors);
        }
        Expr::Call { callee, args } => {
            check_expr_undefined(callee, env, errors);
            for a in args {
                check_expr_undefined(a, env, errors);
            }
        }
        Expr::New { class_name, args } => {
            if !env.contains_key(class_name) {
                errors.push(format!("Undefined class '{}'", class_name));
            }
            for a in args {
                check_expr_undefined(a, env, errors);
            }
        }
        Expr::Index { target, index } => {
            check_expr_undefined(target, env, errors);
            check_expr_undefined(index, env, errors);
        }
        Expr::Member { target, .. } => {
            check_expr_undefined(target, env, errors);
        }
        Expr::FnExpr { params, body } => {
            let mut local = env.clone();
            for (p, _) in params {
                local.insert(p.clone(), InferredType::Any);
            }
            let mut dummy_errs = Vec::new();
            for st in body {
                analyze_stmt(st, &mut local, &mut dummy_errs);
            }
            errors.extend(dummy_errs);
        }
        _ => {}
    }
}

fn find_return_type(stmt: &Stmt, env: &BTreeMap<String, InferredType>) -> Option<InferredType> {
    match stmt {
        Stmt::Return(Some(e)) => Some(infer_expr(e, env)),
        Stmt::Return(None) => Some(InferredType::Null),
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            let mut res: Option<InferredType> = None;
            for st in then_branch {
                if let Some(t) = find_return_type(st, env) {
                    res = Some(match res {
                        Some(prev) => unify(&prev, &t),
                        None => t,
                    });
                }
            }
            if let Some(eb) = else_branch {
                for st in eb {
                    if let Some(t) = find_return_type(st, env) {
                        res = Some(match res {
                            Some(prev) => unify(&prev, &t),
                            None => t,
                        });
                    }
                }
            }
            res
        }
        Stmt::While { body, .. } | Stmt::For { body, .. } => {
            for st in body {
                if let Some(t) = find_return_type(st, env) {
                    return Some(t);
                }
            }
            None
        }
        _ => None,
    }
}

pub fn infer_expr(expr: &Expr, env: &BTreeMap<String, InferredType>) -> InferredType {
    match expr {
        Expr::Number(_) => InferredType::Num,
        Expr::String(_) => InferredType::Str,
        Expr::Bool(_) => InferredType::Bool,
        Expr::Null => InferredType::Null,
        Expr::Array(_) | Expr::Range { .. } => InferredType::Array,
        Expr::Map(_) => InferredType::Map,
        Expr::New { class_name, .. } => InferredType::Class(class_name.clone()),
        Expr::Ident(name) => env.get(name).cloned().unwrap_or(InferredType::Any),
        Expr::FnExpr { params, .. } => InferredType::Func {
            params: params.len(),
            ret: Box::new(InferredType::Any),
        },
        Expr::Unary { op, .. } => match op {
            UnaryOp::Neg | UnaryOp::BitNot => InferredType::Num,
            UnaryOp::Not => InferredType::Bool,
        },
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => unify(&infer_expr(then_expr, env), &infer_expr(else_expr, env)),
        Expr::Binary { left, op, right } => match op {
            BinaryOp::Add => {
                let lt = infer_expr(left, env);
                let rt = infer_expr(right, env);
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
            | BinaryOp::Pow
            | BinaryOp::Div
            | BinaryOp::Mod
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr => InferredType::Num,
            BinaryOp::Eq
            | BinaryOp::Neq
            | BinaryOp::Lt
            | BinaryOp::Gt
            | BinaryOp::Lte
            | BinaryOp::Gte
            | BinaryOp::And
            | BinaryOp::Or => InferredType::Bool,
            BinaryOp::NullCoalesce => unify(&infer_expr(left, env), &infer_expr(right, env)),
        },
        Expr::Call { callee, .. } => {
            if let Expr::Ident(name) = &**callee {
                match name.as_str() {
                    "str" | "upper" | "lower" | "trim" | "join" | "replace" => {
                        return InferredType::Str
                    }
                    "num" | "len" | "abs" | "sqrt" | "floor" | "ceil" | "round" | "sin" | "cos" | "tan"
                    | "min" | "max" | "clock" => return InferredType::Num,
                    "split" | "keys" | "values" | "range" | "map" | "filter" | "reverse"
                    | "sort" => return InferredType::Array,
                    "contains" | "assert" => return InferredType::Bool,
                    _ => {
                        if let Some(InferredType::Func { ret, .. }) = env.get(name) {
                            return (**ret).clone();
                        }
                    }
                }
            }
            InferredType::Any
        }
        _ => InferredType::Any,
    }
}
