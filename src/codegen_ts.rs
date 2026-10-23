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

fn ts_helpers_preamble(body_out: &str) -> String {
    let mut out = String::new();
    if body_out.contains("read_file(")
        || body_out.contains("write_file(")
        || body_out.contains("append_file(")
        || body_out.contains("remove_file(")
        || body_out.contains("file_exists(")
    {
        out.push_str("import * as fs from \"node:fs\";\n");
    }
    if body_out.contains("PI") || body_out.contains(" E") || body_out.contains("(E") {
        out.push_str("const PI: number = Math.PI;\nconst E: number = Math.E;\n");
    }
    if [
        "abs(", "floor(", "ceil(", "round(", "sqrt(", "sin(", "cos(", "tan(", "atan2(", "log(",
        "exp(", "min(", "max(", "pow(",
    ]
    .iter()
    .any(|m| body_out.contains(m))
    {
        out.push_str(
            "const { abs, floor, ceil, round, sqrt, sin, cos, tan, atan2, log, exp, min, max, pow } = Math;\n",
        );
    }
    for (helper, def) in [
        (
            "_print",
            "const _print = (...args: any[]): void => console.log(...args.map((a: any) => typeof a === \"object\" && a !== null ? JSON.stringify(a) : a));\n",
        ),
        (
            "_len",
            "const _len = (x: any): number => Array.isArray(x) || typeof x === \"string\" ? x.length : Object.keys(x || {}).length;\n",
        ),
        (
            "_push",
            "const _push = (arr: any[], v: any): any[] => { arr.push(v); return arr; };\n",
        ),
        ("_pop", "const _pop = (arr: any[]): any => arr.pop();\n"),
        (
            "_get",
            "const _get = (t: any, i: any): any => Array.isArray(t) && typeof i === \"number\" && i < 0 ? t[t.length + i] : t[i];\n",
        ),
        (
            "_set",
            "const _set = (t: any, i: any, v: any): void => { if (Array.isArray(t) && typeof i === \"number\" && i < 0) t[t.length + i] = v; else t[i] = v; };\n",
        ),
        (
            "_keys",
            "const _keys = (obj: Record<string, any>): string[] => Object.keys(obj);\n",
        ),
        (
            "_values",
            "const _values = (obj: Record<string, any>): any[] => Object.values(obj);\n",
        ),
        (
            "_has_key",
            "const _has_key = (obj: Record<string, any>, k: string): boolean => Object.prototype.hasOwnProperty.call(obj || {}, k);\n",
        ),
        (
            "_delete_key",
            "const _delete_key = (obj: Record<string, any>, k: string): boolean => { if (obj && Object.prototype.hasOwnProperty.call(obj, k)) { delete obj[k]; return true; } return false; };\n",
        ),
        (
            "_concat",
            "const _concat = (a: any[], b: any[]): any[] => [...a, ...b];\n",
        ),
        (
            "_contains",
            "const _contains = (x: any, v: any): boolean => Array.isArray(x) || typeof x === \"string\" ? x.includes(v) : Object.prototype.hasOwnProperty.call(x || {}, v);\n",
        ),
        (
            "_index_of",
            "const _index_of = (x: any, v: any): number => (Array.isArray(x) || typeof x === \"string\") ? x.indexOf(v) : -1;\n",
        ),
        (
            "_starts_with",
            "const _starts_with = (s: any, p: any): boolean => String(s).startsWith(String(p));\n",
        ),
        (
            "_ends_with",
            "const _ends_with = (s: any, p: any): boolean => String(s).endsWith(String(p));\n",
        ),
        (
            "_repeat",
            "const _repeat = (s: any, n: number): string => String(s).repeat(Math.max(0, Math.trunc(n)));\n",
        ),
        (
            "_pad_start",
            "const _pad_start = (s: any, w: number, ch = \" \"): string => String(s).padStart(w, ch);\n",
        ),
        (
            "_pad_end",
            "const _pad_end = (s: any, w: number, ch = \" \"): string => String(s).padEnd(w, ch);\n",
        ),
        (
            "_char_code",
            "const _char_code = (s: string): number => s.length > 0 ? (s.codePointAt(0) ?? 0) : 0;\n",
        ),
        (
            "_from_char_code",
            "const _from_char_code = (n: number): string => String.fromCodePoint(Math.max(0, Math.trunc(n)));\n",
        ),
        (
            "_str",
            "const _str = (x: any): string => typeof x === \"object\" && x !== null ? JSON.stringify(x) : String(x);\n",
        ),
        ("_num", "const _num = (x: any): number => Number(x);\n"),
        ("_int", "const _int = (x: any): number => Math.trunc(Number(x));\n"),
        ("_bool", "const _bool = (x: any): boolean => Boolean(x);\n"),
        (
            "_type",
            "const _type = (x: any): string => x === null || x === undefined ? \"null\" : Array.isArray(x) ? \"array\" : typeof x === \"object\" ? \"map\" : typeof x;\n",
        ),
        (
            "_range",
            "const _range = (a: number, b: number, step = 1): number[] => { const r: number[] = []; if (step > 0) { for (let i = a; i < b; i += step) r.push(i); } else if (step < 0) { for (let i = a; i > b; i += step) r.push(i); } return r; };\n",
        ),
        (
            "_map",
            "const _map = (arr: any[], fn: (v: any) => any): any[] => arr.map(fn);\n",
        ),
        (
            "_filter",
            "const _filter = (arr: any[], fn: (v: any) => any): any[] => arr.filter(fn);\n",
        ),
        (
            "_reduce",
            "const _reduce = (arr: any[], fn: (a: any, b: any) => any, init?: any): any => init !== undefined ? arr.reduce(fn, init) : arr.reduce(fn);\n",
        ),
        (
            "_find",
            "const _find = (arr: any[], fn: (v: any) => any): any => { const r = arr.find(fn); return r === undefined ? null : r; };\n",
        ),
        (
            "_any",
            "const _any = (arr: any[], fn: (v: any) => any): boolean => arr.some(fn);\n",
        ),
        (
            "_all",
            "const _all = (arr: any[], fn: (v: any) => any): boolean => arr.every(fn);\n",
        ),
        (
            "_sum",
            "const _sum = (arr: any[]): number => arr.reduce((a: number, b: any) => a + (typeof b === \"number\" ? b : 0), 0);\n",
        ),
        (
            "_avg",
            "const _avg = (arr: any[]): number => arr.length === 0 ? 0 : arr.reduce((a: number, b: any) => a + (typeof b === \"number\" ? b : 0), 0) / arr.length;\n",
        ),
        (
            "_unique",
            "const _unique = (arr: any[]): any[] => { const out: any[] = []; for (const v of arr) if (!out.includes(v)) out.push(v); return out; };\n",
        ),
        (
            "_flat",
            "const _flat = (arr: any[]): any[] => arr.flat(1);\n",
        ),
        (
            "_zip",
            "const _zip = (a: any[], b: any[]): any[] => { const n = Math.min(a.length, b.length), out: any[] = []; for (let i = 0; i < n; i++) out.push([a[i], b[i]]); return out; };\n",
        ),
        (
            "_enumerate",
            "const _enumerate = (arr: any[]): any[] => arr.map((v: any, i: number) => [i, v]);\n",
        ),
        (
            "_clamp",
            "const _clamp = (x: number, lo: number, hi: number): number => Math.min(Math.max(x, lo), hi);\n",
        ),
        (
            "_sign",
            "const _sign = (x: number): number => x > 0 ? 1 : x < 0 ? -1 : 0;\n",
        ),
        (
            "_split",
            "const _split = (s: string, sep: string): string[] => s.split(sep);\n",
        ),
        (
            "_join",
            "const _join = (arr: any[], sep: string): string => arr.join(sep);\n",
        ),
        ("_trim", "const _trim = (s: string): string => s.trim();\n"),
