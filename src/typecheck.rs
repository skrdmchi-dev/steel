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
        "list_dir", "make_dir", "remove_file", "clock", "sleep", "random", "rand_int", "hash",
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
        "list_dir", "make_dir", "remove_file", "clock", "sleep", "random", "rand_int", "hash",
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
