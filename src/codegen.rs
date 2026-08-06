use crate::ast::{AssignTarget, BinaryOp, ClassMethod, Expr, Program, Stmt, UnaryOp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Js,
    Ts,
    Python,
    Rust,
    C,
    Cpp,
    Go,
    Lua,
}

impl Target {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "js" | "javascript" | "node" => Some(Self::Js),
            "ts" | "typescript" => Some(Self::Ts),
            "py" | "python" => Some(Self::Python),
            "rs" | "rust" => Some(Self::Rust),
            "c" => Some(Self::C),
            "cpp" | "c++" | "cxx" => Some(Self::Cpp),
            "go" | "golang" => Some(Self::Go),
            "lua" => Some(Self::Lua),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Js => "JavaScript (Node.js)",
            Self::Ts => "TypeScript",
            Self::Python => "Python 3",
            Self::Rust => "Rust",
            Self::C => "C (C11)",
            Self::Cpp => "C++ (C++20)",
            Self::Go => "Go",
            Self::Lua => "Lua",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Js => "js",
            Self::Ts => "ts",
            Self::Python => "py",
            Self::Rust => "rs",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Go => "go",
            Self::Lua => "lua",
        }
    }
}

/// Lowers `class` declarations into constructor functions + method maps so targets
/// without dynamic class semantics (Rust, C, Go, Lua) support OOP out of the box.
pub(crate) fn desugar_class(name: &str, _parent: &Option<String>, methods: &[ClassMethod]) -> Stmt {
    let mut init_params: Vec<(String, Option<Expr>)> = Vec::new();
    let mut init_body: Vec<Stmt> = Vec::new();
    let mut map_entries: Vec<(String, Expr)> = Vec::new();

    for m in methods {
        if m.name == "init" || m.name == "constructor" {
            init_params = m.params.clone();
            init_body = m.body.clone();
        } else {
            map_entries.push((
                m.name.clone(),
                Expr::FnExpr {
                    params: m.params.clone(),
                    body: m.body.clone(),
                },
            ));
        }
    }

    let mut fn_body = vec![Stmt::Let {
        name: "this".to_string(),
        is_const: false,
        value: Expr::Map(Vec::new()),
    }];
    for (m_name, m_fn) in map_entries {
        fn_body.push(Stmt::Assign {
            target: AssignTarget::Member {
                target: Expr::Ident("this".to_string()),
                field: m_name,
            },
            value: m_fn,
        });
    }
    fn_body.append(&mut init_body);
    fn_body.push(Stmt::Return(Some(Expr::Ident("this".to_string()))));

    Stmt::FnDecl {
        name: name.to_string(),
        params: init_params,
        body: fn_body,
    }
}

pub fn transpile(program: &Program, target: Target) -> String {
    let optimized = crate::optimizer::optimize_program(program);
    match target {
        Target::Js => emit_js(&optimized),
        Target::Ts => crate::codegen_ts::emit_ts_single(&optimized),
        Target::Python => emit_python(&optimized),
        Target::Rust => emit_rust(&optimized),
        Target::C => emit_c(&optimized),
        Target::Cpp => crate::codegen_cpp::emit_cpp_single(&optimized),
        Target::Go => emit_go(&optimized),
        Target::Lua => emit_lua(&optimized),
    }
}

fn indent(level: usize) -> String {
    "    ".repeat(level)
}

fn escape_str(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{:.0}", n)
    } else {
        format!("{}", n)
    }
}

// 1. JAVASCRIPT TARGET
fn emit_js(program: &Program) -> String {
    let mut body_out = String::new();
    for stmt in &program.statements {
        body_out.push_str(&js_stmt(stmt, 0));
    }

    let mut out = String::new();
    if body_out.contains("read_file(")
        || body_out.contains("write_file(")
        || body_out.contains("append_file(")
        || body_out.contains("remove_file(")
        || body_out.contains("file_exists(")
    {
        out.push_str("const fs = require('fs');\n");
    }
    if body_out.contains("PI") || body_out.contains("E") {
        out.push_str("const PI = Math.PI, E = Math.E;\n");
    }
    if ["abs(", "floor(", "ceil(", "round(", "sqrt(", "sin(", "cos(", "tan(", "atan2(", "log(", "exp(", "min(", "max(", "pow("]
        .iter()
        .any(|m| body_out.contains(m))
    {
        out.push_str("const { abs, floor, ceil, round, sqrt, sin, cos, tan, atan2, log, exp, min, max, pow } = Math;\n");
    }
    for (helper, def) in [
        ("__steel_print", "const __steel_print = (...args) => console.log(...args.map(a => typeof a === 'object' && a !== null ? JSON.stringify(a) : a));\n"),
        ("__steel_len", "const __steel_len = (x) => Array.isArray(x) || typeof x === 'string' ? x.length : Object.keys(x || {}).length;\n"),
        ("__steel_push", "const __steel_push = (arr, v) => { arr.push(v); return arr; };\n"),
        ("__steel_pop", "const __steel_pop = (arr) => arr.pop();\n"),
        ("__steel_get", "const __steel_get = (t, i) => Array.isArray(t) && i < 0 ? t[t.length + i] : t[i];\n"),
        ("__steel_set", "const __steel_set = (t, i, v) => { if (Array.isArray(t) && i < 0) t[t.length + i] = v; else t[i] = v; };\n"),
        ("__steel_keys", "const __steel_keys = (obj) => Object.keys(obj);\n"),
        ("__steel_values", "const __steel_values = (obj) => Object.values(obj);\n"),
        ("__steel_has_key", "const __steel_has_key = (obj, k) => Object.prototype.hasOwnProperty.call(obj || {}, k);\n"),
        ("__steel_delete_key", "const __steel_delete_key = (obj, k) => { if (obj && Object.prototype.hasOwnProperty.call(obj, k)) { delete obj[k]; return true; } return false; };\n"),
        ("__steel_concat", "const __steel_concat = (a, b) => [...a, ...b];\n"),
        ("__steel_contains", "const __steel_contains = (x, v) => Array.isArray(x) || typeof x === 'string' ? x.includes(v) : Object.prototype.hasOwnProperty.call(x || {}, v);\n"),
        ("__steel_index_of", "const __steel_index_of = (x, v) => (Array.isArray(x) || typeof x === 'string') ? x.indexOf(v) : -1;\n"),
        ("__steel_starts_with", "const __steel_starts_with = (s, p) => String(s).startsWith(String(p));\n"),
        ("__steel_ends_with", "const __steel_ends_with = (s, p) => String(s).endsWith(String(p));\n"),
        ("__steel_repeat", "const __steel_repeat = (s, n) => String(s).repeat(Math.max(0, Math.trunc(n)));\n"),
        ("__steel_pad_start", "const __steel_pad_start = (s, w, ch = ' ') => String(s).padStart(w, ch);\n"),
        ("__steel_pad_end", "const __steel_pad_end = (s, w, ch = ' ') => String(s).padEnd(w, ch);\n"),
        ("__steel_char_code", "const __steel_char_code = (s) => s.length > 0 ? s.codePointAt(0) : 0;\n"),
        ("__steel_from_char_code", "const __steel_from_char_code = (n) => String.fromCodePoint(Math.max(0, Math.trunc(n)));\n"),
        ("__steel_str", "const __steel_str = (x) => typeof x === 'object' && x !== null ? JSON.stringify(x) : String(x);\n"),
        ("__steel_num", "const __steel_num = (x) => Number(x);\n"),
        ("__steel_int", "const __steel_int = (x) => Math.trunc(Number(x));\n"),
        ("__steel_bool", "const __steel_bool = (x) => Boolean(x);\n"),
        ("__steel_type", "const __steel_type = (x) => x === null || x === undefined ? 'null' : Array.isArray(x) ? 'array' : typeof x === 'object' ? 'map' : typeof x;\n"),
        ("__steel_range", "const __steel_range = (a, b, step = 1) => { const r = []; if (step > 0) { for (let i = a; i < b; i += step) r.push(i); } else if (step < 0) { for (let i = a; i > b; i += step) r.push(i); } return r; };\n"),
        ("__steel_map", "const __steel_map = (arr, fn) => arr.map(fn);\n"),
        ("__steel_filter", "const __steel_filter = (arr, fn) => arr.filter(fn);\n"),
        ("__steel_reduce", "const __steel_reduce = (arr, fn, init) => init !== undefined ? arr.reduce(fn, init) : arr.reduce(fn);\n"),
        ("__steel_find", "const __steel_find = (arr, fn) => { const r = arr.find(fn); return r === undefined ? null : r; };\n"),
        ("__steel_any", "const __steel_any = (arr, fn) => arr.some(fn);\n"),
        ("__steel_all", "const __steel_all = (arr, fn) => arr.every(fn);\n"),
        ("__steel_sum", "const __steel_sum = (arr) => arr.reduce((a, b) => a + (typeof b === 'number' ? b : 0), 0);\n"),
        ("__steel_avg", "const __steel_avg = (arr) => arr.length === 0 ? 0 : arr.reduce((a, b) => a + (typeof b === 'number' ? b : 0), 0) / arr.length;\n"),
        ("__steel_unique", "const __steel_unique = (arr) => { const out = []; for (const v of arr) if (!out.includes(v)) out.push(v); return out; };\n"),
        ("__steel_flat", "const __steel_flat = (arr) => arr.flat(1);\n"),
        ("__steel_zip", "const __steel_zip = (a, b) => { const n = Math.min(a.length, b.length), out = []; for (let i = 0; i < n; i++) out.push([a[i], b[i]]); return out; };\n"),
        ("__steel_enumerate", "const __steel_enumerate = (arr) => arr.map((v, i) => [i, v]);\n"),
        ("__steel_clamp", "const __steel_clamp = (x, lo, hi) => Math.min(Math.max(x, lo), hi);\n"),
        ("__steel_sign", "const __steel_sign = (x) => x > 0 ? 1 : x < 0 ? -1 : 0;\n"),
        ("__steel_split", "const __steel_split = (s, sep) => s.split(sep);\n"),
        ("__steel_join", "const __steel_join = (arr, sep) => arr.join(sep);\n"),
        ("__steel_trim", "const __steel_trim = (s) => s.trim();\n"),
        ("__steel_upper", "const __steel_upper = (s) => s.toUpperCase();\n"),
        ("__steel_lower", "const __steel_lower = (s) => s.toLowerCase();\n"),
        ("__steel_replace", "const __steel_replace = (s, a, b) => s.split(a).join(b);\n"),
        ("__steel_slice", "const __steel_slice = (x, s, e) => x.slice(s, e);\n"),
        ("__steel_reverse", "const __steel_reverse = (x) => Array.isArray(x) ? [...x].reverse() : x.split('').reverse().join('');\n"),
        ("__steel_sort", "const __steel_sort = (x) => [...x].sort((a, b) => String(a) < String(b) ? -1 : String(a) > String(b) ? 1 : 0);\n"),
        ("__steel_json_stringify", "const __steel_json_stringify = (x) => JSON.stringify(x);\n"),
        ("__steel_json_parse", "const __steel_json_parse = (s) => JSON.parse(s);\n"),
        ("__steel_hash", "const __steel_hash = (x) => { const b = Buffer.from(String(x), 'utf8'); let h = 0xcbf29ce484222325n; for (const c of b) { h ^= BigInt(c); h = (h * 0x0100000001b3n) & 0xffffffffffffffffn; } return h.toString(16).padStart(16, '0'); };\n"),
        ("__steel_assert", "const __steel_assert = (c, m = 'Assertion failed') => { if (!c) throw new Error(m); return true; };\n"),
        ("read_file(", "const read_file = (p) => fs.readFileSync(p, 'utf8');\n"),
        ("write_file(", "const write_file = (p, c) => { fs.writeFileSync(p, String(c)); return true; };\n"),
        ("append_file(", "const append_file = (p, c) => { fs.appendFileSync(p, String(c)); return true; };\n"),
        ("remove_file(", "const remove_file = (p) => { fs.unlinkSync(p); return true; };\n"),
        ("file_exists(", "const file_exists = (p) => fs.existsSync(p);\n"),
        ("env(", "const env = (k) => process.env[k] ?? null;\n"),
        ("time_ms(", "const time_ms = () => Date.now();\n"),
        ("clock(", "const clock = () => Date.now() / 1000;\n"),
    ] {
        if body_out.contains(helper) {
            out.push_str(def);
        }
    }
    out.push('\n');
    out.push_str(&body_out);
    out.replace("__steel_", "_steel_").replace("__SteelObj", "_SteelObj")
}

fn js_safe_ident(name: &str) -> String {
    match name {
        "arguments" | "await" | "break" | "case" | "catch" | "class" | "const" | "continue"
        | "debugger" | "default" | "delete" | "do" | "else" | "enum" | "eval" | "export"
        | "extends" | "false" | "finally" | "for" | "function" | "if" | "implements" | "import"
        | "in" | "instanceof" | "interface" | "let" | "new" | "null" | "package" | "private"
        | "protected" | "public" | "return" | "static" | "super" | "switch" | "throw" | "true"
        | "try" | "typeof" | "var" | "void" | "while" | "with" | "yield" => format!("_{}", name),
        _ => name.to_string(),
    }
}

fn js_params(params: &[(String, Option<Expr>)], lvl: usize) -> String {
    params
        .iter()
        .map(|(n, def)| {
            let sn = js_safe_ident(n);
            match def {
                Some(e) => format!("{} = {}", sn, js_expr(e, lvl)),
                None => sn,
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn js_stmt(stmt: &Stmt, lvl: usize) -> String {
    let pad = indent(lvl);
    match stmt {
        Stmt::Import(path) => format!("{}require(\"{}\");\n", pad, escape_str(path)),
        Stmt::Let {
            name,
            is_const,
            value,
        } => {
            let kw = if *is_const { "const" } else { "let" };
            format!("{}{} {} = {};\n", pad, kw, js_safe_ident(name), js_expr(value, lvl))
        }
        Stmt::Assign { target, value } => {
            if let AssignTarget::Index { target, index } = target {
                return format!(
                    "{}__steel_set({}, {}, {});\n",
                    pad,
                    js_expr(target, lvl),
                    js_expr(index, lvl),
                    js_expr(value, lvl)
                );
            }
            let lhs = match target {
                AssignTarget::Ident(n) => js_safe_ident(n),
                AssignTarget::Member { target, field } => {
                    format!("{}.{}", js_expr(target, lvl), field)
                }
                AssignTarget::Index { .. } => unreachable!(),
            };
            format!("{}{} = {};\n", pad, lhs, js_expr(value, lvl))
        }
        Stmt::FnDecl { name, params, body } => {
            let mut s = format!("{}function {}({}) {{\n", pad, name, js_params(params, lvl));
            for st in body {
                s.push_str(&js_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::ClassDecl {
            name,
            parent,
            methods,
        } => {
            let ext = parent
                .as_ref()
                .map(|p| format!(" extends {}", p))
                .unwrap_or_default();
            let mut s = format!("{}class {}{} {{\n", pad, name, ext);
            for m in methods {
                let m_name = if m.name == "init" {
                    "constructor"
                } else {
                    m.name.as_str()
                };
                s.push_str(&format!(
                    "{}    {}({}) {{\n{}        const self = this;\n",
                    pad,
                    m_name,
                    js_params(&m.params, lvl + 1),
                    pad
                ));
                for st in &m.body {
                    s.push_str(&js_stmt(st, lvl + 2));
                }
                s.push_str(&format!("{}    }}\n", pad));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::TryCatch {
            try_body,
            err_var,
            catch_body,
        } => {
            let mut s = format!("{}try {{\n", pad);
            for st in try_body {
                s.push_str(&js_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}}} catch ({}) {{\n", pad, err_var));
            for st in catch_body {
                s.push_str(&js_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::Throw(e) => format!("{}throw {};\n", pad, js_expr(e, lvl)),
        Stmt::Test { name, body } => {
            let mut s = format!("{}// test \"{}\"\n{}{{\n", pad, escape_str(name), pad);
            for st in body {
                s.push_str(&js_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::Defer(inner) => js_stmt(inner, lvl),
        Stmt::Return(expr) => match expr {
            Some(e) => format!("{}return {};\n", pad, js_expr(e, lvl)),
            None => format!("{}return null;\n", pad),
        },
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let mut s = format!("{}if ({}) {{\n", pad, js_expr(cond, lvl));
            for st in then_branch {
                s.push_str(&js_stmt(st, lvl + 1));
            }
            if let Some(eb) = else_branch {
                s.push_str(&format!("{}}} else {{\n", pad));
                for st in eb {
                    s.push_str(&js_stmt(st, lvl + 1));
                }
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::While { cond, body } => {
            let mut s = format!("{}while ({}) {{\n", pad, js_expr(cond, lvl));
            for st in body {
                s.push_str(&js_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::For {
            var,
            iterable,
            body,
        } => {
            let mut s = format!(
                "{}for (const {} of {}) {{\n",
                pad,
                var,
                js_expr(iterable, lvl)
            );
            for st in body {
                s.push_str(&js_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::Break => format!("{}break;\n", pad),
        Stmt::Continue => format!("{}continue;\n", pad),
        Stmt::Expr(e) => format!("{}{};\n", pad, js_expr(e, lvl)),
    }
}

fn js_expr(expr: &Expr, lvl: usize) -> String {
    match expr {
        Expr::Number(n) => fmt_num(*n),
        Expr::String(s) => format!("\"{}\"", escape_str(s)),
        Expr::Bool(b) => b.to_string(),
        Expr::Null => "null".to_string(),
        Expr::Ident(name) => match name.as_str() {
            "print" | "println" => "__steel_print".to_string(),
            "len" => "__steel_len".to_string(),
            "push" => "__steel_push".to_string(),
            "pop" => "__steel_pop".to_string(),
            "keys" => "__steel_keys".to_string(),
            "values" => "__steel_values".to_string(),
            "has_key" => "__steel_has_key".to_string(),
            "delete_key" => "__steel_delete_key".to_string(),
            "concat" => "__steel_concat".to_string(),
            "contains" => "__steel_contains".to_string(),
            "index_of" => "__steel_index_of".to_string(),
            "starts_with" => "__steel_starts_with".to_string(),
            "ends_with" => "__steel_ends_with".to_string(),
            "repeat" => "__steel_repeat".to_string(),
            "pad_start" => "__steel_pad_start".to_string(),
            "pad_end" => "__steel_pad_end".to_string(),
            "char_code" => "__steel_char_code".to_string(),
            "from_char_code" => "__steel_from_char_code".to_string(),
            "str" => "__steel_str".to_string(),
            "num" => "__steel_num".to_string(),
            "int" => "__steel_int".to_string(),
            "bool" => "__steel_bool".to_string(),
            "type" => "__steel_type".to_string(),
            "range" => "__steel_range".to_string(),
            "map" => "__steel_map".to_string(),
            "filter" => "__steel_filter".to_string(),
            "reduce" => "__steel_reduce".to_string(),
            "find" => "__steel_find".to_string(),
            "any" => "__steel_any".to_string(),
            "all" => "__steel_all".to_string(),
            "sum" => "__steel_sum".to_string(),
            "avg" => "__steel_avg".to_string(),
            "unique" => "__steel_unique".to_string(),
            "flat" => "__steel_flat".to_string(),
            "zip" => "__steel_zip".to_string(),
            "enumerate" => "__steel_enumerate".to_string(),
            "clamp" => "__steel_clamp".to_string(),
            "sign" => "__steel_sign".to_string(),
            "split" => "__steel_split".to_string(),
            "join" => "__steel_join".to_string(),
            "trim" => "__steel_trim".to_string(),
            "upper" => "__steel_upper".to_string(),
            "lower" => "__steel_lower".to_string(),
            "replace" => "__steel_replace".to_string(),
            "slice" => "__steel_slice".to_string(),
            "reverse" => "__steel_reverse".to_string(),
            "sort" => "__steel_sort".to_string(),
            "json_stringify" => "__steel_json_stringify".to_string(),
            "json_parse" => "__steel_json_parse".to_string(),
            "hash" => "__steel_hash".to_string(),
            "assert" => "__steel_assert".to_string(),
            _ => js_safe_ident(name),
        },
        Expr::Array(items) => {
            let inner: Vec<String> = items.iter().map(|e| js_expr(e, lvl)).collect();
            format!("[{}]", inner.join(", "))
        }
        Expr::Map(entries) => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("\"{}\": {}", escape_str(k), js_expr(v, lvl)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
        Expr::New { class_name, args } => {
            let arg_strs: Vec<String> = args.iter().map(|a| js_expr(a, lvl)).collect();
            format!("(new {}({}))", class_name, arg_strs.join(", "))
        }
        Expr::Ternary {
            cond,
            then_expr,
            else_expr,
        } => format!(
            "({} ? {} : {})",
            js_expr(cond, lvl),
            js_expr(then_expr, lvl),
            js_expr(else_expr, lvl)
        ),
        Expr::Binary { left, op, right } => {
            let op_str = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Pow => "**",
                BinaryOp::Div => "/",
                BinaryOp::Mod => "%",
                BinaryOp::Eq => "===",
                BinaryOp::Neq => "!==",
                BinaryOp::Lt => "<",
                BinaryOp::Gt => ">",
                BinaryOp::Lte => "<=",
                BinaryOp::Gte => ">=",
                BinaryOp::And => "&&",
                BinaryOp::Or => "||",
                BinaryOp::BitAnd => "&",
                BinaryOp::BitOr => "|",
                BinaryOp::BitXor => "^",
                BinaryOp::Shl => "<<",
                BinaryOp::Shr => ">>",
                BinaryOp::NullCoalesce => "??",
            };
            format!("({} {} {})", js_expr(left, lvl), op_str, js_expr(right, lvl))
        }
        Expr::Unary { op, expr } => {
            let op_str = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
                UnaryOp::BitNot => "~",
            };
            format!("({}{})", op_str, js_expr(expr, lvl))
        }
        Expr::Call { callee, args } => {
            let arg_strs: Vec<String> = args.iter().map(|a| js_expr(a, lvl)).collect();
            format!("{}({})", js_expr(callee, lvl), arg_strs.join(", "))
        }
        Expr::Index { target, index } => {
            format!("__steel_get({}, {})", js_expr(target, lvl), js_expr(index, lvl))
        }
        Expr::Member { target, field } => format!("{}.{}", js_expr(target, lvl), field),
        Expr::FnExpr { params, body } => {
            let mut s = format!("(({}) => {{\n", js_params(params, lvl));
            for st in body {
                s.push_str(&js_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}}})", indent(lvl)));
            s
        }
        Expr::Range { start, end } => {
            format!(
                "__steel_range({}, {})",
                js_expr(start, lvl),
                js_expr(end, lvl)
            )
        }
        Expr::Spread(inner) => format!("...{}", js_expr(inner, lvl)),
    }
}

// 2. PYTHON TARGET (On-Demand Minimal Helpers)
fn emit_python(program: &Program) -> String {
    let mut fn_counter = 0;
    let mut body_out = String::new();
    for stmt in &program.statements {
        body_out.push_str(&py_stmt(stmt, 0, &mut fn_counter));
    }

    let mut out = String::new();
    if body_out.contains("__steel_str")
        || body_out.contains("__steel_print")
        || body_out.contains("__steel_join")
        || body_out.contains("__steel_add")
        || body_out.contains("__steel_json_")
    {
        out.push_str("import json\n");
    }
    if ["sqrt(", "sin(", "cos(", "tan(", "atan2(", "log(", "exp(", "floor(", "ceil(", "PI", "E"]
        .iter()
        .any(|m| body_out.contains(m))
    {
        out.push_str("import math\nPI, E = math.pi, math.e\nsqrt, sin, cos, tan, atan2, log, exp, floor, ceil = math.sqrt, math.sin, math.cos, math.tan, math.atan2, math.log, math.exp, math.floor, math.ceil\n");
    }
    if body_out.contains("__steel_reduce") {
        out.push_str("import functools\n");
    }
    if body_out.contains("__SteelObj") || body_out.contains("__steel_json_parse") {
        out.push_str("class __SteelObj(dict):\n    def __getattribute__(self, k):\n        if dict.__contains__(self, k): return dict.__getitem__(self, k)\n        return object.__getattribute__(self, k)\n    __getattr__ = dict.get\n    __setattr__ = dict.__setitem__\n");
    }
    if body_out.contains("__steel_str")
        || body_out.contains("__steel_print")
        || body_out.contains("__steel_join")
        || body_out.contains("__steel_add")
        || body_out.contains("__steel_sort")
        || body_out.contains("__steel_hash")
    {
        out.push_str(
            "def __steel_str(x):\n\
             \x20   if x is None: return 'null'\n\
             \x20   if isinstance(x, bool): return 'true' if x else 'false'\n\
             \x20   if isinstance(x, float) and x.is_integer(): return str(int(x))\n\
             \x20   if isinstance(x, (list, dict)): return json.dumps(x, separators=(',', ':'))\n\
             \x20   return str(x)\n",
        );
    }
    for (helper, def) in [
        ("__steel_add", "def __steel_add(a, b): return (__steel_str(a) + __steel_str(b)) if (isinstance(a, str) or isinstance(b, str)) else (a + b)\n"),
        ("__steel_print", "def __steel_print(*args): print(*(__steel_str(a) for a in args))\n"),
        ("__steel_push", "def __steel_push(arr, val): arr.append(val); return arr\n"),
        ("__steel_pop", "def __steel_pop(arr): return arr.pop() if arr else None\n"),
        ("__steel_get", "def __steel_get(t, i): return t.get(i, None) if isinstance(t, dict) else (t[int(i)] if -len(t) <= int(i) < len(t) else None)\n"),
        ("__steel_keys", "def __steel_keys(obj): return list(obj.keys())\n"),
        ("__steel_values", "def __steel_values(obj): return list(obj.values())\n"),
        ("__steel_has_key", "def __steel_has_key(obj, k): return isinstance(obj, dict) and k in obj\n"),
        ("__steel_delete_key", "def __steel_delete_key(obj, k):\n    if isinstance(obj, dict) and k in obj:\n        del obj[k]\n        return True\n    return False\n"),
        ("__steel_concat", "def __steel_concat(a, b): return list(a) + list(b)\n"),
        ("__steel_contains", "def __steel_contains(x, v): return v in x\n"),
        ("__steel_index_of", "def __steel_index_of(x, v):\n    if isinstance(x, str): return x.find(str(v))\n    if isinstance(x, list):\n        try: return x.index(v)\n        except ValueError: return -1\n    return -1\n"),
        ("__steel_repeat", "def __steel_repeat(s, n): return str(s) * max(0, int(n))\n"),
        ("__steel_pad_start", "def __steel_pad_start(s, w, ch=' '):\n    s, ch = str(s), str(ch)\n    need = max(0, int(w) - len(s))\n    if need == 0 or not ch: return s\n    return (ch * (need // len(ch) + 1))[:need] + s\n"),
        ("__steel_pad_end", "def __steel_pad_end(s, w, ch=' '):\n    s, ch = str(s), str(ch)\n    need = max(0, int(w) - len(s))\n    if need == 0 or not ch: return s\n    return s + (ch * (need // len(ch) + 1))[:need]\n"),
        ("__steel_char_code", "def __steel_char_code(s): return ord(s[0]) if len(s) > 0 else 0\n"),
        ("__steel_from_char_code", "def __steel_from_char_code(n): return chr(max(0, int(n)))\n"),
        ("__steel_int", "def __steel_int(x): return int(float(x))\n"),
        ("__steel_bool", "def __steel_bool(x): return bool(x)\n"),
        ("__steel_range", "def __steel_range(a, b, step=1): return list(range(int(a), int(b), int(step)))\n"),
        ("__steel_map", "def __steel_map(arr, fn): return [fn(x) for x in arr]\n"),
        ("__steel_filter", "def __steel_filter(arr, fn): return [x for x in arr if fn(x)]\n"),
        ("__steel_reduce", "def __steel_reduce(arr, fn, *init): return functools.reduce(fn, arr, *init)\n"),
        ("__steel_find", "def __steel_find(arr, fn):\n    for x in arr:\n        if fn(x): return x\n    return None\n"),
        ("__steel_any", "def __steel_any(arr, fn): return any(fn(x) for x in arr)\n"),
        ("__steel_all", "def __steel_all(arr, fn): return all(fn(x) for x in arr)\n"),
        ("__steel_sum", "def __steel_sum(arr): return sum(x for x in arr if isinstance(x, (int, float)) and not isinstance(x, bool))\n"),
        ("__steel_avg", "def __steel_avg(arr): return 0 if not arr else sum(x for x in arr if isinstance(x, (int, float)) and not isinstance(x, bool)) / len(arr)\n"),
        ("__steel_unique", "def __steel_unique(arr):\n    out = []\n    for v in arr:\n        if v not in out: out.append(v)\n    return out\n"),
        ("__steel_flat", "def __steel_flat(arr):\n    out = []\n    for v in arr:\n        if isinstance(v, list): out.extend(v)\n        else: out.append(v)\n    return out\n"),
        ("__steel_zip", "def __steel_zip(a, b): return [[x, y] for x, y in zip(a, b)]\n"),
        ("__steel_enumerate", "def __steel_enumerate(arr): return [[i, v] for i, v in enumerate(arr)]\n"),
        ("__steel_clamp", "def __steel_clamp(x, lo, hi): return min(max(x, lo), hi)\n"),
        ("__steel_sign", "def __steel_sign(x): return 1 if x > 0 else (-1 if x < 0 else 0)\n"),
        ("__steel_split", "def __steel_split(s, sep): return s.split(sep) if sep != '' else list(s)\n"),
        ("__steel_join", "def __steel_join(arr, sep): return sep.join(__steel_str(x) for x in arr)\n"),
        ("__steel_trim", "def __steel_trim(s): return s.strip()\n"),
        ("__steel_upper", "def __steel_upper(s): return s.upper()\n"),
        ("__steel_lower", "def __steel_lower(s): return s.lower()\n"),
        ("__steel_replace", "def __steel_replace(s, a, b): return s.replace(a, b)\n"),
        ("__steel_slice", "def __steel_slice(x, s, e=None): return x[int(s):None if e is None else int(e)]\n"),
        ("__steel_starts_with", "def __steel_starts_with(s, p): return str(s).startswith(str(p))\n"),
        ("__steel_ends_with", "def __steel_ends_with(s, p): return str(s).endswith(str(p))\n"),
        ("__steel_reverse", "def __steel_reverse(x): return list(reversed(x)) if isinstance(x, list) else x[::-1]\n"),
        ("__steel_sort", "def __steel_sort(x): return sorted(x, key=__steel_str)\n"),
        ("__steel_json_stringify", "def __steel_json_stringify(x): return json.dumps(x, separators=(',', ':'))\n"),
        ("__steel_json_parse", "def __steel_json_parse(s): return json.loads(s, object_hook=__SteelObj)\n"),
        ("__steel_hash", "def __steel_hash(x):\n    h = 0xcbf29ce484222325\n    for b in __steel_str(x).encode('utf-8'):\n        h ^= b\n        h = (h * 0x0100000001b3) & 0xffffffffffffffff\n    return f'{h:016x}'\n"),
        ("__steel_assert", "def __steel_assert(c, m='Assertion failed'):\n    if not c: raise RuntimeError(m)\n    return True\n"),
        ("read_file(", "def read_file(p):\n    with open(p, 'r', encoding='utf-8') as f: return f.read()\n"),
        ("write_file(", "def write_file(p, c):\n    with open(p, 'w', encoding='utf-8') as f: f.write(str(c))\n    return True\n"),
        ("append_file(", "def append_file(p, c):\n    with open(p, 'a', encoding='utf-8') as f: f.write(str(c))\n    return True\n"),
        ("remove_file(", "import os as _os_rm\ndef remove_file(p): _os_rm.remove(p); return True\n"),
        ("file_exists(", "import os as _os_fe\ndef file_exists(p): return _os_fe.path.exists(p)\n"),
        ("env(", "import os as _os_env\ndef env(k): return _os_env.environ.get(k, None)\n"),
        ("time_ms(", "import time as _time_ms\ndef time_ms(): return int(_time_ms.time() * 1000)\n"),
        ("clock(", "import time as _time_clk\ndef clock(): return _time_clk.time()\n"),
        ("__steel_type", "def __steel_type(x):\n    if x is None: return 'null'\n    if isinstance(x, bool): return 'boolean'\n    if isinstance(x, (int, float)): return 'number'\n    if isinstance(x, str): return 'string'\n    if isinstance(x, list): return 'array'\n    if isinstance(x, dict): return 'map'\n    return 'function'\n"),
    ] {
        if body_out.contains(helper) {
            out.push_str(def);
        }
    }
    out.push('\n');
    out.push_str(&body_out);
    out.replace("__steel_", "_steel_").replace("__SteelObj", "_SteelObj")
}

fn py_safe_ident(name: &str) -> String {
    match name {
        "False" | "None" | "True" | "and" | "as" | "assert" | "async" | "await" | "break"
        | "class" | "continue" | "def" | "del" | "elif" | "else" | "except" | "finally" | "for"
        | "from" | "global" | "if" | "import" | "in" | "is" | "lambda" | "nonlocal" | "not"
        | "or" | "pass" | "raise" | "return" | "try" | "while" | "with" | "yield" => {
            format!("_{}", name)
        }
        _ => name.to_string(),
    }
}

fn py_params(params: &[(String, Option<Expr>)], lvl: usize, fn_counter: &mut usize) -> String {
    params
        .iter()
        .map(|(n, def)| {
            let sn = py_safe_ident(n);
            match def {
                Some(e) => {
                    let (_, v) = py_expr(e, lvl, fn_counter);
                    format!("{}={}", sn, v)
                }
                None => sn,
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn py_stmt(stmt: &Stmt, lvl: usize, fn_counter: &mut usize) -> String {
    let pad = indent(lvl);
    match stmt {
        Stmt::Import(path) => format!("{}# import \"{}\"\n", pad, escape_str(path)),
        Stmt::Let { name, value, .. } => {
            let (pre, val_str) = py_expr(value, lvl, fn_counter);
            format!("{}{}{} = {}\n", pre, pad, py_safe_ident(name), val_str)
        }
        Stmt::Assign { target, value } => {
            let (pre, val_str) = py_expr(value, lvl, fn_counter);
            let lhs = match target {
                AssignTarget::Ident(n) => py_safe_ident(n),
                AssignTarget::Index { target, index } => {
                    let (_, t) = py_expr(target, lvl, fn_counter);
                    let (_, i) = py_expr(index, lvl, fn_counter);
                    format!("{}[{}]", t, i)
                }
                AssignTarget::Member { target, field } => {
                    let (_, t) = py_expr(target, lvl, fn_counter);
                    format!("{}.{}", t, field)
                }
            };
            format!("{}{}{} = {}\n", pre, pad, lhs, val_str)
        }
        Stmt::FnDecl { name, params, body } => {
            let p_str = py_params(params, lvl, fn_counter);
            let mut s = format!("{}def {}({}):\n", pad, py_safe_ident(name), p_str);
            if body.is_empty() {
                s.push_str(&format!("{}    pass\n", pad));
            } else {
                for st in body {
                    s.push_str(&py_stmt(st, lvl + 1, fn_counter));
                }
            }
            s
        }
        Stmt::ClassDecl {
            name,
            parent,
            methods,
        } => {
            let base = parent.as_deref().unwrap_or("object");
            let mut s = format!("{}class {}({}):\n", pad, name, base);
            if methods.is_empty() {
                s.push_str(&format!("{}    pass\n", pad));
            } else {
                for m in methods {
                    let m_name = if m.name == "init" || m.name == "constructor" {
                        "__init__"
                    } else {
                        m.name.as_str()
                    };
                    let p_str = py_params(&m.params, lvl + 1, fn_counter);
                    let full_params = if p_str.is_empty() {
                        "self".to_string()
                    } else {
                        format!("self, {}", p_str)
                    };
                    s.push_str(&format!(
                        "{}    def {}({}):\n{}        this = self\n",
                        pad, m_name, full_params, pad
                    ));
                    for st in &m.body {
                        s.push_str(&py_stmt(st, lvl + 2, fn_counter));
                    }
                }
            }
            s
        }
        Stmt::TryCatch {
            try_body,
            err_var,
            catch_body,
        } => {
            let mut s = format!("{}try:\n", pad);
            if try_body.is_empty() {
                s.push_str(&format!("{}    pass\n", pad));
            } else {
                for st in try_body {
                    s.push_str(&py_stmt(st, lvl + 1, fn_counter));
                }
            }
            s.push_str(&format!(
                "{}except Exception as __e:\n{}    {} = str(__e)\n",
                pad, pad, err_var
            ));
            for st in catch_body {
                s.push_str(&py_stmt(st, lvl + 1, fn_counter));
            }
            s
        }
        Stmt::Throw(e) => {
            let (pre, val) = py_expr(e, lvl, fn_counter);
            format!("{}{}raise RuntimeError(__steel_str({}))\n", pre, pad, val)
        }
        Stmt::Test { name, body } => {
            let mut s = format!("{}# test \"{}\"\n", pad, escape_str(name));
            for st in body {
                s.push_str(&py_stmt(st, lvl, fn_counter));
            }
            s
        }
        Stmt::Defer(inner) => py_stmt(inner, lvl, fn_counter),
        Stmt::Return(expr) => match expr {
            Some(e) => {
                let (pre, val) = py_expr(e, lvl, fn_counter);
                format!("{}{}return {}\n", pre, pad, val)
            }
            None => format!("{}return None\n", pad),
        },
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let (pre, c) = py_expr(cond, lvl, fn_counter);
            let mut s = format!("{}{}if {}:\n", pre, pad, c);
            if then_branch.is_empty() {
                s.push_str(&format!("{}    pass\n", pad));
            } else {
                for st in then_branch {
                    s.push_str(&py_stmt(st, lvl + 1, fn_counter));
                }
            }
            if let Some(eb) = else_branch {
                s.push_str(&format!("{}else:\n", pad));
                if eb.is_empty() {
                    s.push_str(&format!("{}    pass\n", pad));
                } else {
                    for st in eb {
                        s.push_str(&py_stmt(st, lvl + 1, fn_counter));
                    }
                }
            }
            s
        }
        Stmt::While { cond, body } => {
            let (pre, c) = py_expr(cond, lvl, fn_counter);
            let mut s = format!("{}{}while {}:\n", pre, pad, c);
            if body.is_empty() {
                s.push_str(&format!("{}    pass\n", pad));
            } else {
                for st in body {
                    s.push_str(&py_stmt(st, lvl + 1, fn_counter));
                }
            }
            s
        }
        Stmt::For {
            var,
            iterable,
            body,
        } => {
            if let Expr::Range { start, end } = iterable {
                let (mut pre, s_val) = py_expr(start, lvl, fn_counter);
                let (p2, e_val) = py_expr(end, lvl, fn_counter);
                pre.push_str(&p2);
                let mut s = format!("{}{}for {} in range(int({}), int({})):\n", pre, pad, var, s_val, e_val);
                if body.is_empty() {
                    s.push_str(&format!("{}    pass\n", pad));
                } else {
                    for st in body {
                        s.push_str(&py_stmt(st, lvl + 1, fn_counter));
                    }
                }
                return s;
            }
            let (pre, it) = py_expr(iterable, lvl, fn_counter);
            let mut s = format!("{}{}for {} in {}:\n", pre, pad, var, it);
            if body.is_empty() {
                s.push_str(&format!("{}    pass\n", pad));
            } else {
                for st in body {
                    s.push_str(&py_stmt(st, lvl + 1, fn_counter));
                }
            }
            s
        }
        Stmt::Break => format!("{}break\n", pad),
        Stmt::Continue => format!("{}continue\n", pad),
        Stmt::Expr(e) => {
            let (pre, val) = py_expr(e, lvl, fn_counter);
            format!("{}{}{}\n", pre, pad, val)
        }
    }
}

fn py_expr(expr: &Expr, lvl: usize, fn_counter: &mut usize) -> (String, String) {
    match expr {
        Expr::Number(n) => (String::new(), fmt_num(*n)),
        Expr::String(s) => (String::new(), format!("\"{}\"", escape_str(s))),
        Expr::Bool(b) => (String::new(), if *b { "True" } else { "False" }.to_string()),
        Expr::Null => (String::new(), "None".to_string()),
        Expr::Ident(name) => {
            let mapped = match name.as_str() {
                "print" | "println" => "__steel_print",
                "len" => "len",
                "push" => "__steel_push",
                "pop" => "__steel_pop",
                "keys" => "__steel_keys",
                "values" => "__steel_values",
                "has_key" => "__steel_has_key",
                "delete_key" => "__steel_delete_key",
                "concat" => "__steel_concat",
                "contains" => "__steel_contains",
                "index_of" => "__steel_index_of",
                "starts_with" => "__steel_starts_with",
                "ends_with" => "__steel_ends_with",
                "repeat" => "__steel_repeat",
                "pad_start" => "__steel_pad_start",
                "pad_end" => "__steel_pad_end",
                "char_code" => "__steel_char_code",
                "from_char_code" => "__steel_from_char_code",
                "str" => "__steel_str",
                "num" => "float",
                "int" => "__steel_int",
                "bool" => "__steel_bool",
                "type" => "__steel_type",
                "range" => "__steel_range",
                "map" => "__steel_map",
                "filter" => "__steel_filter",
                "reduce" => "__steel_reduce",
                "find" => "__steel_find",
                "any" => "__steel_any",
                "all" => "__steel_all",
                "sum" => "__steel_sum",
                "avg" => "__steel_avg",
                "unique" => "__steel_unique",
                "flat" => "__steel_flat",
                "zip" => "__steel_zip",
                "enumerate" => "__steel_enumerate",
                "clamp" => "__steel_clamp",
                "sign" => "__steel_sign",
                "split" => "__steel_split",
                "join" => "__steel_join",
                "trim" => "__steel_trim",
                "upper" => "__steel_upper",
                "lower" => "__steel_lower",
                "replace" => "__steel_replace",
                "slice" => "__steel_slice",
                "reverse" => "__steel_reverse",
                "sort" => "__steel_sort",
                "json_stringify" => "__steel_json_stringify",
                "json_parse" => "__steel_json_parse",
                "hash" => "__steel_hash",
                "assert" => "__steel_assert",
                other => return (String::new(), py_safe_ident(other)),
            };
            (String::new(), mapped.to_string())
        }
        Expr::Array(items) => {
            let mut pre = String::new();
            let mut parts = Vec::new();
            for item in items {
                let (p, v) = py_expr(item, lvl, fn_counter);
                pre.push_str(&p);
                parts.push(v);
            }
            (pre, format!("[{}]", parts.join(", ")))
        }
        Expr::Map(entries) => {
            let mut pre = String::new();
            let mut parts = Vec::new();
            for (k, val) in entries {
                let (p, v) = py_expr(val, lvl, fn_counter);
                pre.push_str(&p);
                parts.push(format!("\"{}\": {}", escape_str(k), v));
            }
            (pre, format!("__SteelObj({{{}}})", parts.join(", ")))
        }
        Expr::New { class_name, args } => {
            let mut pre = String::new();
            let mut arg_strs = Vec::new();
            for a in args {
                let (p, v) = py_expr(a, lvl, fn_counter);
                pre.push_str(&p);
                arg_strs.push(v);
            }
            (pre, format!("{}({})", class_name, arg_strs.join(", ")))
        }
        Expr::Ternary {
            cond,
            then_expr,
            else_expr,
        } => {
            let (mut pre, c) = py_expr(cond, lvl, fn_counter);
            let (p1, t) = py_expr(then_expr, lvl, fn_counter);
            let (p2, e) = py_expr(else_expr, lvl, fn_counter);
            pre.push_str(&p1);
            pre.push_str(&p2);
            (pre, format!("({} if {} else {})", t, c, e))
        }
        Expr::Binary { left, op, right } => {
            let (mut pre, l) = py_expr(left, lvl, fn_counter);
            let (p2, r) = py_expr(right, lvl, fn_counter);
            pre.push_str(&p2);
            if matches!(op, BinaryOp::NullCoalesce) {
                return (
                    pre,
                    format!("({} if {} is not None else {})", l, l, r),
                );
            }
            if matches!(op, BinaryOp::Add) {
                let both_num_lit = matches!(**left, Expr::Number(_)) && matches!(**right, Expr::Number(_));
                if !both_num_lit {
                    return (pre, format!("__steel_add({}, {})", l, r));
                }
            }
            let op_str = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Pow => "**",
                BinaryOp::Div => "/",
                BinaryOp::Mod => "%",
                BinaryOp::Eq => "==",
                BinaryOp::Neq => "!=",
                BinaryOp::Lt => "<",
                BinaryOp::Gt => ">",
                BinaryOp::Lte => "<=",
                BinaryOp::Gte => ">=",
                BinaryOp::And => "and",
                BinaryOp::Or => "or",
                BinaryOp::BitAnd => "&",
                BinaryOp::BitOr => "|",
                BinaryOp::BitXor => "^",
                BinaryOp::Shl => "<<",
                BinaryOp::Shr => ">>",
                BinaryOp::NullCoalesce => unreachable!(),
            };
            (pre, format!("({} {} {})", l, op_str, r))
        }
        Expr::Unary { op, expr } => {
            let (pre, e) = py_expr(expr, lvl, fn_counter);
            let op_str = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "not ",
                UnaryOp::BitNot => "~",
            };
            (pre, format!("({}{})", op_str, e))
        }
        Expr::Call { callee, args } => {
            let (mut pre, c) = py_expr(callee, lvl, fn_counter);
            let mut arg_strs = Vec::new();
            for a in args {
                let (p, v) = py_expr(a, lvl, fn_counter);
                pre.push_str(&p);
                arg_strs.push(v);
            }
            (pre, format!("{}({})", c, arg_strs.join(", ")))
        }
        Expr::Index { target, index } => {
            let (mut pre, t) = py_expr(target, lvl, fn_counter);
            let (p2, i) = py_expr(index, lvl, fn_counter);
            pre.push_str(&p2);
            (pre, format!("__steel_get({}, {})", t, i))
        }
        Expr::Member { target, field } => {
            let (pre, t) = py_expr(target, lvl, fn_counter);
            (pre, format!("{}.{}", t, field))
        }
        Expr::FnExpr { params, body } => {
            *fn_counter += 1;
            let fn_name = format!("__anon_fn_{}", *fn_counter);
            let pad = indent(lvl);
            let p_str = py_params(params, lvl, fn_counter);
            let mut def = format!("{}def {}({}):\n", pad, fn_name, p_str);
            if body.is_empty() {
                def.push_str(&format!("{}    pass\n", pad));
            } else {
                for st in body {
                    def.push_str(&py_stmt(st, lvl + 1, fn_counter));
                }
            }
            (def, fn_name)
        }
        Expr::Range { start, end } => {
            let (mut pre, s) = py_expr(start, lvl, fn_counter);
            let (p2, e) = py_expr(end, lvl, fn_counter);
            pre.push_str(&p2);
            (pre, format!("list(range(int({}), int({})))", s, e))
        }
        Expr::Spread(inner) => {
            let (pre, s) = py_expr(inner, lvl, fn_counter);
            (pre, format!("*{}", s))
        }
    }
}

// 3. LUA TARGET (On-Demand Minimal Helpers)
fn lua_safe_ident(name: &str) -> String {
    match name {
        "and" | "do" | "elseif" | "end" | "function" | "goto" | "local" | "nil" | "not" | "or"
        | "repeat" | "then" | "until" => format!("_st_{}", name),
        _ => name.to_string(),
    }
}

fn emit_lua(program: &Program) -> String {
    let mut fwd_names: Vec<String> = Vec::new();
    for stmt in &program.statements {
        match stmt {
            Stmt::FnDecl { name, .. } | Stmt::ClassDecl { name, .. } => {
                fwd_names.push(lua_safe_ident(name));
            }
            _ => {}
        }
    }
    let mut body_out = String::new();
    if !fwd_names.is_empty() {
        body_out.push_str(&format!("local {}\n", fwd_names.join(", ")));
    }
    for stmt in &program.statements {
        body_out.push_str(&lua_stmt(stmt, 0));
    }
    let mut out = String::new();
    if body_out.contains("PI") || body_out.contains("E") {
        out.push_str("local PI, E = math.pi, math.exp(1)\n");
    }
    if ["sqrt(", "abs(", "floor(", "ceil(", "sin(", "cos(", "tan(", "atan2(", "log(", "exp(", "min(", "max("].iter().any(|m| body_out.contains(m)) {
        out.push_str("local abs, floor, ceil, sqrt, sin, cos, tan, log, exp, min, max = math.abs, math.floor, math.ceil, math.sqrt, math.sin, math.cos, math.tan, math.log, math.exp, math.min, math.max\nlocal function atan2(y, x) return math.atan(y, x) end\n");
    }
    if body_out.contains("__steel_str")
        || body_out.contains("__steel_print")
        || body_out.contains("__steel_add")
        || body_out.contains("__steel_join")
        || body_out.contains("__steel_sort")
        || body_out.contains("__steel_hash")
    {
        out.push_str(
            "local function __steel_str(v)\n\
             \x20   if v == nil then return 'null' end\n\
             \x20   if type(v) == 'number' then\n\
             \x20       if math.type(v) == 'float' and v == math.floor(v) and math.abs(v) < 9007199254740992 then\n\
             \x20           return string.format('%.0f', v)\n\
             \x20       end\n\
             \x20       return tostring(v)\n\
             \x20   end\n\
             \x20   if type(v) == 'table' then\n\
             \x20       local parts = {}\n\
             \x20       for k, val in pairs(v) do table.insert(parts, tostring(k) .. ':' .. __steel_str(val)) end\n\
             \x20       return '{' .. table.concat(parts, ', ') .. '}'\n\
             \x20   end\n\
             \x20   return tostring(v)\n\
             end\n",
        );
    }
    for (helper, def) in [
        ("__steel_add", "local function __steel_add(a, b) if type(a) == 'string' or type(b) == 'string' then return __steel_str(a) .. __steel_str(b) end return a + b end\n"),
        ("__steel_print", "local function __steel_print(...)\n    local args, out = {...}, {}\n    for i = 1, select('#', ...) do table.insert(out, __steel_str(args[i])) end\n    print(table.concat(out, ' '))\nend\n"),
        ("__steel_range", "local function __steel_range(a, b, step)\n    step = step or 1\n    local t = {}\n    if step > 0 then for i = a, b - 1, step do table.insert(t, i) end\n    elseif step < 0 then for i = a, b + 1, step do table.insert(t, i) end end\n    return t\nend\n"),
        ("__steel_push", "local function __steel_push(t, v) table.insert(t, v); return t end\n"),
        ("__steel_pop", "local function __steel_pop(t) return table.remove(t) end\n"),
        ("__steel_get", "local function __steel_get(t, i) if type(i) == 'number' then if i < 0 then i = #t + i end return t[i + 1] end return t[i] end\n"),
        ("__steel_set", "local function __steel_set(t, i, v) if type(i) == 'number' then if i < 0 then i = #t + i end t[i + 1] = v else t[i] = v end end\n"),
        ("__steel_len", "local function __steel_len(t) if type(t) == 'string' then return #t end if type(t) == 'table' then local n = #t; if n > 0 then return n end local c = 0; for _ in pairs(t) do c = c + 1 end return c end return 0 end\n"),
        ("__steel_has_key", "local function __steel_has_key(t, k) return type(t) == 'table' and t[k] ~= nil end\n"),
        ("__steel_delete_key", "local function __steel_delete_key(t, k) if type(t) == 'table' and t[k] ~= nil then t[k] = nil; return true end return false end\n"),
        ("__steel_concat", "local function __steel_concat(a, b) local r = {}; for _, v in ipairs(a) do table.insert(r, v) end for _, v in ipairs(b) do table.insert(r, v) end return r end\n"),
        ("__steel_index_of", "local function __steel_index_of(t, v) if type(t) == 'string' then local p = string.find(t, tostring(v), 1, true); return p and (p - 1) or -1 end if type(t) == 'table' then for i, x in ipairs(t) do if x == v then return i - 1 end end end return -1 end\n"),
        ("__steel_repeat", "local function __steel_repeat(s, n) return string.rep(tostring(s), math.max(0, math.floor(n))) end\n"),
        ("__steel_pad_start", "local function __steel_pad_start(s, w, ch) s = tostring(s); ch = ch or ' '; local need = math.floor(w) - #s; if need <= 0 or #ch == 0 then return s end return string.sub(string.rep(ch, math.floor(need / #ch) + 1), 1, need) .. s end\n"),
        ("__steel_pad_end", "local function __steel_pad_end(s, w, ch) s = tostring(s); ch = ch or ' '; local need = math.floor(w) - #s; if need <= 0 or #ch == 0 then return s end return s .. string.sub(string.rep(ch, math.floor(need / #ch) + 1), 1, need) end\n"),
        ("__steel_char_code", "local function __steel_char_code(s) return #s > 0 and string.byte(s, 1) or 0 end\n"),
        ("__steel_from_char_code", "local function __steel_from_char_code(n) return utf8.char(math.max(0, math.floor(n))) end\n"),
        ("__steel_int", "local function __steel_int(x) local n = tonumber(x) or 0; return n >= 0 and math.floor(n) or math.ceil(n) end\n"),
        ("__steel_bool", "local function __steel_bool(x) return not (x == nil or x == false or x == 0 or x == '') end\n"),
        ("__steel_type", "local function __steel_type(x) if x == nil then return 'null' end local t = type(x); if t == 'table' then for k, _ in pairs(x) do if type(k) ~= 'number' then return 'map' end end return 'array' end return t end\n"),
        ("__steel_map", "local function __steel_map(t, fn) local r = {}; for i, v in ipairs(t) do r[i] = fn(v) end; return r end\n"),
        ("__steel_filter", "local function __steel_filter(t, fn) local r = {}; for _, v in ipairs(t) do if fn(v) then table.insert(r, v) end end; return r end\n"),
        ("__steel_reduce", "local function __steel_reduce(t, fn, init) local acc, s; if init ~= nil then acc = init; s = 1 else acc = t[1]; s = 2 end; for i = s, #t do acc = fn(acc, t[i]) end; return acc end\n"),
        ("__steel_find", "local function __steel_find(t, fn) for _, v in ipairs(t) do if fn(v) then return v end end return nil end\n"),
        ("__steel_any", "local function __steel_any(t, fn) for _, v in ipairs(t) do if fn(v) then return true end end return false end\n"),
        ("__steel_all", "local function __steel_all(t, fn) for _, v in ipairs(t) do if not fn(v) then return false end end return true end\n"),
        ("__steel_sum", "local function __steel_sum(t) local s = 0; for _, v in ipairs(t) do if type(v) == 'number' then s = s + v end end return s end\n"),
        ("__steel_avg", "local function __steel_avg(t) if #t == 0 then return 0 end local s = 0; for _, v in ipairs(t) do if type(v) == 'number' then s = s + v end end return s / #t end\n"),
        ("__steel_unique", "local function __steel_unique(t) local r = {}; for _, v in ipairs(t) do local f = false; for _, u in ipairs(r) do if u == v then f = true; break end end if not f then table.insert(r, v) end end return r end\n"),
        ("__steel_flat", "local function __steel_flat(t) local r = {}; for _, v in ipairs(t) do if type(v) == 'table' then for _, u in ipairs(v) do table.insert(r, u) end else table.insert(r, v) end end return r end\n"),
        ("__steel_zip", "local function __steel_zip(a, b) local r = {}; local n = math.min(#a, #b); for i = 1, n do table.insert(r, {a[i], b[i]}) end return r end\n"),
        ("__steel_enumerate", "local function __steel_enumerate(t) local r = {}; for i, v in ipairs(t) do table.insert(r, {i - 1, v}) end return r end\n"),
        ("__steel_clamp", "local function __steel_clamp(x, lo, hi) if x < lo then return lo end if x > hi then return hi end return x end\n"),
        ("__steel_sign", "local function __steel_sign(x) if x > 0 then return 1 elseif x < 0 then return -1 else return 0 end end\n"),
        ("__steel_upper", "local function __steel_upper(s) return string.upper(s) end\n"),
        ("__steel_lower", "local function __steel_lower(s) return string.lower(s) end\n"),
        ("__steel_trim", "local function __steel_trim(s) return (s:gsub('^%s*(.-)%s*$', '%1')) end\n"),
        ("__steel_contains", "local function __steel_contains(s, sub) if type(s) == 'table' then if s[sub] ~= nil then return true end for _, v in ipairs(s) do if v == sub then return true end end return false end return string.find(tostring(s), tostring(sub), 1, true) ~= nil end\n"),
        ("__steel_starts_with", "local function __steel_starts_with(s, p) s = tostring(s); p = tostring(p); return string.sub(s, 1, #p) == p end\n"),
        ("__steel_ends_with", "local function __steel_ends_with(s, p) s = tostring(s); p = tostring(p); return p == '' or string.sub(s, -#p) == p end\n"),
        ("__steel_split", "local function __steel_split(s, sep) local r = {}; if sep == '' then for i = 1, #s do table.insert(r, string.sub(s, i, i)) end return r end local i = 1; while true do local a, b = string.find(s, sep, i, true); if not a then table.insert(r, string.sub(s, i)); break end table.insert(r, string.sub(s, i, a - 1)); i = b + 1 end return r end\n"),
        ("__steel_join", "local function __steel_join(t, sep) local o = {}; for i, v in ipairs(t) do o[i] = __steel_str(v) end return table.concat(o, sep) end\n"),
        ("__steel_replace", "local function __steel_replace(s, a, b) local r, i = {}, 1; while true do local x, y = string.find(s, a, i, true); if not x then table.insert(r, string.sub(s, i)); break end table.insert(r, string.sub(s, i, x - 1)); table.insert(r, b); i = y + 1 end return table.concat(r) end\n"),
        ("__steel_reverse", "local function __steel_reverse(t) if type(t) == 'string' then return string.reverse(t) end local r = {}; for i = #t, 1, -1 do table.insert(r, t[i]) end return r end\n"),
        ("__steel_keys", "local function __steel_keys(t) local r = {}; for k, _ in pairs(t) do table.insert(r, k) end return r end\n"),
        ("__steel_values", "local function __steel_values(t) local r = {}; for _, v in pairs(t) do table.insert(r, v) end return r end\n"),
        ("__steel_slice", "local function __steel_slice(t, s, e) if type(t) == 'string' then return string.sub(t, s + 1, e) end local r = {}; local last = e or #t; for i = s + 1, last do if t[i] ~= nil then table.insert(r, t[i]) end end return r end\n"),
        ("__steel_sort", "local function __steel_sort(t) local r = {}; for i, v in ipairs(t) do r[i] = v end table.sort(r, function(a, b) return __steel_str(a) < __steel_str(b) end); return r end\n"),
        ("__steel_json_stringify", "local function __steel_json_esc(s) return '\"' .. s:gsub('\\\\', '\\\\\\\\'):gsub('\"', '\\\\\"'):gsub('\\n', '\\\\n'):gsub('\\r', '\\\\r'):gsub('\\t', '\\\\t') .. '\"' end\nlocal function __steel_json_stringify(v)\n    if v == nil then return 'null' end\n    local t = type(v)\n    if t == 'boolean' then return tostring(v) end\n    if t == 'number' then if math.type(v) == 'float' and v == math.floor(v) and math.abs(v) < 9007199254740992 then return string.format('%.0f', v) end return tostring(v) end\n    if t == 'string' then return __steel_json_esc(v) end\n    if t == 'table' then\n        local is_arr, max_k = true, 0\n        for k, _ in pairs(v) do if type(k) ~= 'number' or k < 1 or k ~= math.floor(k) then is_arr = false; break end if k > max_k then max_k = k end end\n        if is_arr then\n            local p = {}; for i = 1, max_k do p[i] = __steel_json_stringify(v[i]) end; return '[' .. table.concat(p, ',') .. ']'\n        end\n        local p = {}; for k, val in pairs(v) do table.insert(p, __steel_json_esc(tostring(k)) .. ':' .. __steel_json_stringify(val)) end\n        return '{' .. table.concat(p, ',') .. '}'\n    end\n    return 'null'\nend\n"),
        ("__steel_json_parse", "local function __steel_json_parse(s)\n    local i, n = 1, #s\n    local function skip() while i <= n and s:sub(i,i):match('%s') do i = i + 1 end end\n    local parse_val\n    local function parse_str()\n        i = i + 1; local out = {}\n        while i <= n do\n            local c = s:sub(i,i); i = i + 1\n            if c == '\"' then return table.concat(out) end\n            if c == '\\\\' then\n                local e = s:sub(i,i); i = i + 1\n                if e == 'n' then table.insert(out, '\\n') elseif e == 'r' then table.insert(out, '\\r') elseif e == 't' then table.insert(out, '\\t') else table.insert(out, e) end\n            else table.insert(out, c) end\n        end\n        return table.concat(out)\n    end\n    parse_val = function()\n        skip(); local c = s:sub(i,i)\n        if c == '\"' then return parse_str() end\n        if c == '[' then\n            i = i + 1; local arr = {}; local idx = 1; skip()\n            if s:sub(i,i) == ']' then i = i + 1; return arr end\n            while i <= n do arr[idx] = parse_val(); idx = idx + 1; skip(); if s:sub(i,i) == ',' then i = i + 1 else i = i + 1; break end end\n            return arr\n        end\n        if c == '{' then\n            i = i + 1; local obj = {}; skip()\n            if s:sub(i,i) == '}' then i = i + 1; return obj end\n            while i <= n do skip(); local k = parse_str(); skip(); i = i + 1; obj[k] = parse_val(); skip(); if s:sub(i,i) == ',' then i = i + 1 else i = i + 1; break end end\n            return obj\n        end\n        if s:sub(i, i + 3) == 'true' then i = i + 4; return true end\n        if s:sub(i, i + 4) == 'false' then i = i + 5; return false end\n        if s:sub(i, i + 3) == 'null' then i = i + 4; return nil end\n        local num_s = s:match('^-?%d+%.?%d*[eE]?[+-]?%d*', i)\n        i = i + #num_s; return tonumber(num_s)\n    end\n    return parse_val()\nend\n"),
        ("__steel_hash", "local function __steel_hash(x) local s = __steel_str(x); local h = 0xcbf29ce484222325; for i = 1, #s do h = (h ~ string.byte(s, i)) * 0x0100000001b3 end; return string.format('%016x', h) end\n"),
        ("__steel_assert", "local function __steel_assert(c, m) if not c then error(m or 'Assertion failed') end return true end\n"),
        ("round(", "local function round(x) return x >= 0 and math.floor(x + 0.5) or math.ceil(x - 0.5) end\n"),
        ("pow(", "local function pow(a, b) return a ^ b end\n"),
        ("read_file(", "local function read_file(p) local f = assert(io.open(p, 'r')); local c = f:read('*a'); f:close(); return c end\n"),
        ("write_file(", "local function write_file(p, c) local f = assert(io.open(p, 'w')); f:write(tostring(c)); f:close(); return true end\n"),
        ("append_file(", "local function append_file(p, c) local f = assert(io.open(p, 'a')); f:write(tostring(c)); f:close(); return true end\n"),
        ("remove_file(", "local function remove_file(p) os.remove(p); return true end\n"),
        ("file_exists(", "local function file_exists(p) local f = io.open(p, 'r'); if f then f:close(); return true end return false end\n"),
        ("env(", "local function env(k) return os.getenv(k) end\n"),
        ("time_ms(", "local function time_ms() return math.floor(os.time() * 1000) end\n"),
        ("clock(", "local function clock() return os.time() end\n"),
    ] {
        if body_out.contains(helper) {
            out.push_str(def);
        }
    }
    out.push('\n');
    out.push_str(&body_out);
    out.replace("__steel_", "_steel_").replace("__SteelObj", "_SteelObj")
}

fn lua_try_stmt(stmt: &Stmt, lvl: usize) -> String {
    let pad = indent(lvl);
    match stmt {
        Stmt::Return(val) => {
            let v = val.as_ref().map(|e| lua_expr(e, lvl)).unwrap_or_else(|| "nil".to_string());
            format!("{}return \"__STEEL_RET__\", {}\n", pad, v)
        }
        Stmt::If { cond, then_branch, else_branch } => {
            let mut s = format!("{}if {} then\n", pad, lua_expr(cond, lvl));
            for st in then_branch { s.push_str(&lua_try_stmt(st, lvl + 1)); }
            if let Some(eb) = else_branch {
                s.push_str(&format!("{}else\n", pad));
                for st in eb { s.push_str(&lua_try_stmt(st, lvl + 1)); }
            }
            s.push_str(&format!("{}end\n", pad));
            s
        }
        Stmt::While { cond, body } => {
            let mut s = format!("{}while {} do\n", pad, lua_expr(cond, lvl));
            for st in body { s.push_str(&lua_try_stmt(st, lvl + 1)); }
            s.push_str(&format!("{}end\n", pad));
            s
        }
        other => lua_stmt(other, lvl),
    }
}

fn lua_stmt(stmt: &Stmt, lvl: usize) -> String {
    let pad = indent(lvl);
    match stmt {
        Stmt::Import(path) => format!("{}require(\"{}\")\n", pad, escape_str(path)),
        Stmt::ClassDecl {
            name,
            parent,
            methods,
        } => lua_stmt(&desugar_class(name, parent, methods), lvl),
        Stmt::TryCatch {
            try_body,
            err_var,
            catch_body,
        } => {
            let mut s = format!("{}local __ok, {}, __try_val = pcall(function()\n", pad, err_var);
            for st in try_body {
                s.push_str(&lua_try_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}end)\n{}if __ok and {} == \"__STEEL_RET__\" then return __try_val end\n{}if not __ok then\n", pad, pad, err_var, pad));
            for st in catch_body {
                s.push_str(&lua_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}end\n", pad));
            s
        }
        Stmt::Throw(e) => format!("{}error({}, 0)\n", pad, lua_expr(e, lvl)),
        Stmt::Test { name, body } => {
            let mut s = format!("{}-- test \"{}\"\n{}do\n", pad, escape_str(name), pad);
            for st in body {
                s.push_str(&lua_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}end\n", pad));
            s
        }
        Stmt::Defer(inner) => lua_stmt(inner, lvl),
        Stmt::Let { name, value, .. } => {
            format!("{}local {} = {}\n", pad, lua_safe_ident(name), lua_expr(value, lvl))
        }
        Stmt::Assign { target, value } => {
            if let AssignTarget::Index { target, index } = target {
                return format!(
                    "{}__steel_set({}, {}, {})\n",
                    pad,
                    lua_expr(target, lvl),
                    lua_expr(index, lvl),
                    lua_expr(value, lvl)
                );
            }
            let lhs = match target {
                AssignTarget::Ident(n) => lua_safe_ident(n),
                AssignTarget::Member { target, field } => {
                    format!("{}.{}", lua_expr(target, lvl), field)
                }
                AssignTarget::Index { .. } => unreachable!(),
            };
            format!("{}{} = {}\n", pad, lhs, lua_expr(value, lvl))
        }
        Stmt::FnDecl { name, params, body } => {
            let p_names: Vec<String> = params.iter().map(|(p, _)| lua_safe_ident(p)).collect();
            let mut s = if lvl == 0 {
                format!("{}{} = function({})\n", pad, lua_safe_ident(name), p_names.join(", "))
            } else {
                format!("{}local function {}({})\n", pad, lua_safe_ident(name), p_names.join(", "))
            };
            for (p, def) in params {
                if let Some(d) = def {
                    s.push_str(&format!(
                        "{}    if {} == nil then {} = {} end\n",
                        pad,
                        p,
                        p,
                        lua_expr(d, lvl + 1)
                    ));
                }
            }
            for st in body {
                s.push_str(&lua_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}end\n", pad));
            s
        }
        Stmt::Return(expr) => match expr {
            Some(e) => format!("{}do return {} end\n", pad, lua_expr(e, lvl)),
            None => format!("{}do return nil end\n", pad),
        },
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let mut s = format!("{}if {} then\n", pad, lua_expr(cond, lvl));
            for st in then_branch {
                s.push_str(&lua_stmt(st, lvl + 1));
            }
            if let Some(eb) = else_branch {
                s.push_str(&format!("{}else\n", pad));
                for st in eb {
                    s.push_str(&lua_stmt(st, lvl + 1));
                }
            }
            s.push_str(&format!("{}end\n", pad));
            s
        }
        Stmt::While { cond, body } => {
            let mut body_s = String::new();
            for st in body {
                body_s.push_str(&lua_stmt(st, lvl + 1));
            }
            let mut s = format!("{}while {} do\n{}", pad, lua_expr(cond, lvl), body_s);
            if body_s.contains("goto __continue") {
                s.push_str(&format!("{}    ::__continue::\n", pad));
            }
            s.push_str(&format!("{}end\n", pad));
            s
        }
        Stmt::For {
            var,
            iterable,
            body,
        } => {
            let mut body_s = String::new();
            for st in body {
                body_s.push_str(&lua_stmt(st, lvl + 1));
            }
            let mut s = format!(
                "{}for _, {} in ipairs({}) do\n{}",
                pad,
                var,
                lua_expr(iterable, lvl),
                body_s
            );
            if body_s.contains("goto __continue") {
                s.push_str(&format!("{}    ::__continue::\n", pad));
            }
            s.push_str(&format!("{}end\n", pad));
            s
        }
        Stmt::Break => format!("{}break\n", pad),
        Stmt::Continue => format!("{}goto __continue\n", pad),
        Stmt::Expr(e) => format!("{}{}\n", pad, lua_expr(e, lvl)),
    }
}

fn lua_expr(expr: &Expr, lvl: usize) -> String {
    match expr {
        Expr::Number(n) => fmt_num(*n),
        Expr::String(s) => format!("\"{}\"", escape_str(s)),
        Expr::Bool(b) => b.to_string(),
        Expr::Null => "nil".to_string(),
        Expr::Ident(name) => match name.as_str() {
            "print" | "println" => "__steel_print".to_string(),
            "len" => "__steel_len".to_string(),
            "push" => "__steel_push".to_string(),
            "pop" => "__steel_pop".to_string(),
            "has_key" => "__steel_has_key".to_string(),
            "delete_key" => "__steel_delete_key".to_string(),
            "concat" => "__steel_concat".to_string(),
            "index_of" => "__steel_index_of".to_string(),
            "repeat" => "__steel_repeat".to_string(),
            "pad_start" => "__steel_pad_start".to_string(),
            "pad_end" => "__steel_pad_end".to_string(),
            "char_code" => "__steel_char_code".to_string(),
            "from_char_code" => "__steel_from_char_code".to_string(),
            "str" => "__steel_str".to_string(),
            "num" => "tonumber".to_string(),
            "int" => "__steel_int".to_string(),
            "bool" => "__steel_bool".to_string(),
            "type" => "__steel_type".to_string(),
            "range" => "__steel_range".to_string(),
            "map" => "__steel_map".to_string(),
            "filter" => "__steel_filter".to_string(),
            "reduce" => "__steel_reduce".to_string(),
            "find" => "__steel_find".to_string(),
            "any" => "__steel_any".to_string(),
            "all" => "__steel_all".to_string(),
            "sum" => "__steel_sum".to_string(),
            "avg" => "__steel_avg".to_string(),
            "unique" => "__steel_unique".to_string(),
            "flat" => "__steel_flat".to_string(),
            "zip" => "__steel_zip".to_string(),
            "enumerate" => "__steel_enumerate".to_string(),
            "clamp" => "__steel_clamp".to_string(),
            "sign" => "__steel_sign".to_string(),
            "upper" => "__steel_upper".to_string(),
            "lower" => "__steel_lower".to_string(),
            "trim" => "__steel_trim".to_string(),
            "contains" => "__steel_contains".to_string(),
            "starts_with" => "__steel_starts_with".to_string(),
            "ends_with" => "__steel_ends_with".to_string(),
            "split" => "__steel_split".to_string(),
            "join" => "__steel_join".to_string(),
            "replace" => "__steel_replace".to_string(),
            "reverse" => "__steel_reverse".to_string(),
            "keys" => "__steel_keys".to_string(),
            "values" => "__steel_values".to_string(),
            "slice" => "__steel_slice".to_string(),
            "sort" => "__steel_sort".to_string(),
            "json_stringify" => "__steel_json_stringify".to_string(),
            "json_parse" => "__steel_json_parse".to_string(),
            "hash" => "__steel_hash".to_string(),
            "assert" => "__steel_assert".to_string(),
            _ => lua_safe_ident(name),
        },
        Expr::Array(items) => {
            let inner: Vec<String> = items.iter().map(|e| lua_expr(e, lvl)).collect();
            format!("{{{}}}", inner.join(", "))
        }
        Expr::Map(entries) => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("[\"{}\"] = {}", escape_str(k), lua_expr(v, lvl)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
        Expr::New { class_name, args } => {
            let arg_strs: Vec<String> = args.iter().map(|a| lua_expr(a, lvl)).collect();
            format!("{}({})", class_name, arg_strs.join(", "))
        }
        Expr::Ternary {
            cond,
            then_expr,
            else_expr,
        } => format!(
            "(({}) and ({}) or ({}))",
            lua_expr(cond, lvl),
