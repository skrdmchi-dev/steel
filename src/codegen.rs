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
            lua_expr(then_expr, lvl),
            lua_expr(else_expr, lvl)
        ),
        Expr::Binary { left, op, right } => {
            let l = lua_expr(left, lvl);
            let r = lua_expr(right, lvl);
            if matches!(op, BinaryOp::NullCoalesce) {
                return format!("(({} ~= nil) and {} or {})", l, l, r);
            }
            if matches!(op, BinaryOp::Add) {
                let both_num_lit = matches!(**left, Expr::Number(_)) && matches!(**right, Expr::Number(_));
                if !both_num_lit {
                    return format!("__steel_add({}, {})", l, r);
                }
            }
            let op_str = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Pow => "^",
                BinaryOp::Div => "/",
                BinaryOp::Mod => "%",
                BinaryOp::Eq => "==",
                BinaryOp::Neq => "~=",
                BinaryOp::Lt => "<",
                BinaryOp::Gt => ">",
                BinaryOp::Lte => "<=",
                BinaryOp::Gte => ">=",
                BinaryOp::And => "and",
                BinaryOp::Or => "or",
                BinaryOp::BitAnd => "&",
                BinaryOp::BitOr => "|",
                BinaryOp::BitXor => "~",
                BinaryOp::Shl => "<<",
                BinaryOp::Shr => ">>",
                BinaryOp::NullCoalesce => unreachable!(),
            };
            format!("({} {} {})", l, op_str, r)
        }
        Expr::Unary { op, expr } => {
            let op_str = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "not ",
                UnaryOp::BitNot => "~",
            };
            format!("({}{})", op_str, lua_expr(expr, lvl))
        }
        Expr::Call { callee, args } => {
            let arg_strs: Vec<String> = args.iter().map(|a| lua_expr(a, lvl)).collect();
            format!("{}({})", lua_expr(callee, lvl), arg_strs.join(", "))
        }
        Expr::Index { target, index } => {
            format!("__steel_get({}, {})", lua_expr(target, lvl), lua_expr(index, lvl))
        }
        Expr::Member { target, field } => format!("{}.{}", lua_expr(target, lvl), field),
        Expr::FnExpr { params, body } => {
            let p_names: Vec<String> = params.iter().map(|(p, _)| lua_safe_ident(p)).collect();
            let mut s = format!("function({})\n", p_names.join(", "));
            let inner_pad = indent(lvl + 1);
            for (p, def) in params {
                if let Some(d) = def {
                    let sp = lua_safe_ident(p);
                    s.push_str(&format!("{}if {} == nil then {} = {} end\n", inner_pad, sp, sp, lua_expr(d, lvl + 1)));
                }
            }
            for st in body {
                s.push_str(&lua_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}end", indent(lvl)));
            s
        }
        Expr::Range { start, end } => {
            format!(
                "__steel_range({}, {})",
                lua_expr(start, lvl),
                lua_expr(end, lvl)
            )
        }
        Expr::Spread(inner) => format!("table.unpack({})", lua_expr(inner, lvl)),
    }
}

// 4. RUST TARGET (with self-contained dynamic SVal runtime!)
fn emit_rust(program: &Program) -> String {
    if let Some(pure_rs) = try_emit_human_rust(program) {
        return pure_rs;
    }
    let mut out = String::from(
        "// Generated by the Steel Transpiler (Target: Rust)\n\
         #![allow(unused_variables,unused_mut,dead_code,unreachable_code,unused_parens,non_snake_case)]\n\
         use std::cell::RefCell;\n\
         use std::collections::BTreeMap;\n\
         use std::fmt;\n\
         use std::ops::{Add, Sub, Mul, Div, Rem, Neg, Not};\n\
         use std::rc::Rc;\n\n\
         #[derive(Clone)]\n\
         pub enum SVal {\n\
         \x20   Null,\n\
         \x20   Bool(bool),\n\
         \x20   Num(f64),\n\
         \x20   Str(String),\n\
         \x20   Arr(Rc<RefCell<Vec<SVal>>>),\n\
         \x20   Map(Rc<RefCell<BTreeMap<String, SVal>>>),\n\
         \x20   Func(Rc<dyn Fn(Vec<SVal>) -> SVal>),\n\
         }\n\n\
         impl SVal {\n\
         \x20   pub fn is_truthy(&self) -> bool {\n\
         \x20       match self {\n\
         \x20           SVal::Null => false,\n\
         \x20           SVal::Bool(b) => *b,\n\
         \x20           SVal::Num(n) => *n != 0.0,\n\
         \x20           SVal::Str(s) => !s.is_empty(),\n\
         \x20           SVal::Arr(a) => !a.borrow().is_empty(),\n\
         \x20           SVal::Map(m) => !m.borrow().is_empty(),\n\
         \x20           SVal::Func(_) => true,\n\
         \x20       }\n\
         \x20   }\n\
         \x20   pub fn as_num(&self) -> f64 {\n\
         \x20       match self { SVal::Num(n) => *n, SVal::Bool(true) => 1.0, SVal::Str(s) => s.trim().parse().unwrap_or(0.0), _ => 0.0 }\n\
         \x20   }\n\
         \x20   pub fn call(&self, args: Vec<SVal>) -> SVal {\n\
         \x20       if let SVal::Func(f) = self { f(args) } else { panic!(\"Attempted to call non-function: {}\", self) }\n\
         \x20   }\n\
         \x20   pub fn get(&self, idx: SVal) -> SVal {\n\
         \x20       match (self, idx) {\n\
         \x20           (SVal::Arr(a), SVal::Num(n)) => { let b = a.borrow(); let mut i = n as isize; if i < 0 { i += b.len() as isize; } if i >= 0 { b.get(i as usize).cloned().unwrap_or(SVal::Null) } else { SVal::Null } },\n\
         \x20           (SVal::Map(m), SVal::Str(k)) => m.borrow().get(&k).cloned().unwrap_or(SVal::Null),\n\
         \x20           _ => SVal::Null,\n\
         \x20       }\n\
         \x20   }\n\
         \x20   pub fn set(&self, idx: SVal, val: SVal) {\n\
         \x20       match (self, idx) {\n\
         \x20           (SVal::Arr(a), SVal::Num(n)) => { let mut b = a.borrow_mut(); let mut i = n as isize; if i < 0 { i += b.len() as isize; } if i >= 0 && (i as usize) < b.len() { b[i as usize] = val; } }\n\
         \x20           (SVal::Map(m), SVal::Str(k)) => { m.borrow_mut().insert(k, val); }\n\
         \x20           _ => {}\n\
         \x20       }\n\
         \x20   }\n\
         \x20   pub fn iter_vec(&self) -> Vec<SVal> {\n\
         \x20       match self { SVal::Arr(a) => a.borrow().clone(), _ => vec![] }\n\
         \x20   }\n\
         }\n\n\
         impl fmt::Display for SVal {\n\
         \x20   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n\
         \x20       match self {\n\
         \x20           SVal::Null => write!(f, \"null\"),\n\
         \x20           SVal::Bool(b) => write!(f, \"{}\", b),\n\
         \x20           SVal::Num(n) => if n.fract() == 0.0 { write!(f, \"{:.0}\", n) } else { write!(f, \"{}\", n) },\n\
         \x20           SVal::Str(s) => write!(f, \"{}\", s),\n\
         \x20           SVal::Arr(a) => {\n\
         \x20               let items: Vec<String> = a.borrow().iter().map(|v| v.to_string()).collect();\n\
         \x20               write!(f, \"[{}]\", items.join(\", \"))\n\
         \x20           }\n\
         \x20           SVal::Map(m) => {\n\
         \x20               let items: Vec<String> = m.borrow().iter().map(|(k, v)| format!(\"{}: {}\", k, v)).collect();\n\
         \x20               write!(f, \"{{{}}}\", items.join(\", \"))\n\
         \x20           }\n\
         \x20           SVal::Func(_) => write!(f, \"<fn>\"),\n\
         \x20       }\n\
         \x20   }\n\
         }\n\n\
         impl PartialEq for SVal {\n\
         \x20   fn eq(&self, other: &Self) -> bool {\n\
         \x20       match (self, other) {\n\
         \x20           (SVal::Null, SVal::Null) => true,\n\
         \x20           (SVal::Bool(a), SVal::Bool(b)) => a == b,\n\
         \x20           (SVal::Num(a), SVal::Num(b)) => a == b,\n\
         \x20           (SVal::Str(a), SVal::Str(b)) => a == b,\n\
         \x20           _ => false,\n\
         \x20       }\n\
         \x20   }\n\
         }\n\n\
         impl Add for SVal { type Output = SVal; fn add(self, rhs: SVal) -> SVal {\n\
         \x20   match (&self, &rhs) {\n\
         \x20       (SVal::Str(_), _) | (_, SVal::Str(_)) => SVal::Str(format!(\"{}{}\", self, rhs)),\n\
         \x20       _ => SVal::Num(self.as_num() + rhs.as_num()),\n\
         \x20   }\n\
         }}\n\
         impl Sub for SVal { type Output = SVal; fn sub(self, rhs: SVal) -> SVal { SVal::Num(self.as_num() - rhs.as_num()) } }\n\
         impl Mul for SVal { type Output = SVal; fn mul(self, rhs: SVal) -> SVal { SVal::Num(self.as_num() * rhs.as_num()) } }\n\
         impl Div for SVal { type Output = SVal; fn div(self, rhs: SVal) -> SVal { SVal::Num(self.as_num() / rhs.as_num()) } }\n\
         impl Rem for SVal { type Output = SVal; fn rem(self, rhs: SVal) -> SVal { SVal::Num(self.as_num() % rhs.as_num()) } }\n\
         impl Neg for SVal { type Output = SVal; fn neg(self) -> SVal { SVal::Num(-self.as_num()) } }\n\
         impl Not for SVal { type Output = SVal; fn not(self) -> SVal { SVal::Bool(!self.is_truthy()) } }\n\n\
         fn __steel_range(a: SVal, b: SVal) -> SVal {\n\
         \x20   let start = a.as_num() as i64;\n\
         \x20   let end = b.as_num() as i64;\n\
         \x20   let v: Vec<SVal> = (start..end).map(|i| SVal::Num(i as f64)).collect();\n\
         \x20   SVal::Arr(Rc::new(RefCell::new(v)))\n\
         }\n\n",
    );

    let mut top_fn_names = std::collections::HashSet::new();
    let mut top_var_names = std::collections::HashSet::new();
    for stmt in &program.statements {
        match stmt {
            Stmt::FnDecl { name, .. } | Stmt::ClassDecl { name, .. } => {
                top_fn_names.insert(name.clone());
            }
            Stmt::Let { name, .. } => {
                top_var_names.insert(name.clone());
            }
            Stmt::Assign { target: AssignTarget::Ident(name), .. } => {
                top_var_names.insert(name.clone());
            }
            _ => {}
        }
    }
    let mut fn_used_globals = std::collections::BTreeSet::new();
    for stmt in &program.statements {
        if let Stmt::FnDecl { params, body, .. } = stmt {
            let mut locals: std::collections::HashSet<String> =
                params.iter().map(|(p, _)| p.clone()).collect();
            rs_collect_local_decls(body, &mut locals);
            let mut refs = std::collections::HashSet::new();
            rs_collect_ident_refs_stmts(body, &mut refs);
            for r in refs {
                if top_var_names.contains(&r) && !locals.contains(&r) && !top_fn_names.contains(&r) {
                    fn_used_globals.insert(r);
                }
            }
        }
    }
    if !fn_used_globals.is_empty() {
        out.push_str("thread_local! {\n");
        for g in &fn_used_globals {
            out.push_str(&format!(
                "    static __STEEL_G_{}: RefCell<SVal> = RefCell::new(SVal::Null);\n",
                g
            ));
        }
        out.push_str("}\n\n");
    }
    RS_GLOBALS.with(|gs| {
        *gs.borrow_mut() = fn_used_globals.iter().cloned().collect();
    });
    let mut top_fns = String::new();
    let mut main_body = String::new();
    for stmt in &program.statements {
        match stmt {
            Stmt::FnDecl { name, params, body } => {
                top_fns.push_str(&rs_top_fn(name, params, body, &top_fn_names));
            }
            Stmt::ClassDecl {
                name,
                parent,
                methods,
            } => {
                let desugared = desugar_class(name, parent, methods);
                if let Stmt::FnDecl { name, params, body } = desugared {
                    top_fns.push_str(&rs_top_fn(&name, &params, &body, &top_fn_names));
                }
            }
            other => {
                main_body.push_str(&rs_stmt(other, 1, &top_fn_names));
            }
        }
    }
    RS_GLOBALS.with(|gs| gs.borrow_mut().clear());

    out.push_str(&top_fns);
    out.push_str("fn main() {\n");
    out.push_str(&rs_builtins_preamble(&main_body));
    out.push_str(&main_body);
    out.push_str("}\n");
    out
}

thread_local! {
    static RS_GLOBALS: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}

fn rs_is_global(name: &str) -> bool {
    RS_GLOBALS.with(|gs| gs.borrow().contains(name))
}

fn rs_collect_local_decls(stmts: &[Stmt], out: &mut std::collections::HashSet<String>) {
    for st in stmts {
        match st {
            Stmt::Let { name, .. } => { out.insert(name.clone()); }
            Stmt::For { var, body, .. } => { out.insert(var.clone()); rs_collect_local_decls(body, out); }
            Stmt::If { then_branch, else_branch, .. } => {
                rs_collect_local_decls(then_branch, out);
                if let Some(eb) = else_branch { rs_collect_local_decls(eb, out); }
            }
            Stmt::While { body, .. } => rs_collect_local_decls(body, out),
            Stmt::TryCatch { try_body, err_var, catch_body } => {
                rs_collect_local_decls(try_body, out);
                out.insert(err_var.clone());
                rs_collect_local_decls(catch_body, out);
            }
            _ => {}
        }
    }
}

fn rs_collect_ident_refs_expr(expr: &Expr, out: &mut std::collections::HashSet<String>) {
    match expr {
        Expr::Ident(n) => { out.insert(n.clone()); }
        Expr::Array(items) => for e in items { rs_collect_ident_refs_expr(e, out); },
        Expr::Map(entries) => for (_, v) in entries { rs_collect_ident_refs_expr(v, out); },
        Expr::Binary { left, right, .. } => { rs_collect_ident_refs_expr(left, out); rs_collect_ident_refs_expr(right, out); }
        Expr::Unary { expr, .. } | Expr::Spread(expr) => rs_collect_ident_refs_expr(expr, out),
        Expr::Ternary { cond, then_expr, else_expr } => {
            rs_collect_ident_refs_expr(cond, out);
            rs_collect_ident_refs_expr(then_expr, out);
            rs_collect_ident_refs_expr(else_expr, out);
        }
        Expr::Call { callee, args } => {
            rs_collect_ident_refs_expr(callee, out);
            for a in args { rs_collect_ident_refs_expr(a, out); }
        }
        Expr::New { args, .. } => for a in args { rs_collect_ident_refs_expr(a, out); },
        Expr::Index { target, index } => { rs_collect_ident_refs_expr(target, out); rs_collect_ident_refs_expr(index, out); }
        Expr::Member { target, .. } => rs_collect_ident_refs_expr(target, out),
        Expr::FnExpr { params, body } => {
            let mut inner_refs = std::collections::HashSet::new();
            rs_collect_ident_refs_stmts(body, &mut inner_refs);
            let mut inner_bound: std::collections::HashSet<String> =
                params.iter().map(|(p, _)| p.clone()).collect();
            rs_collect_local_decls(body, &mut inner_bound);
            for r in inner_refs {
                if !inner_bound.contains(&r) {
                    out.insert(r);
                }
            }
        }
        Expr::Range { start, end } => { rs_collect_ident_refs_expr(start, out); rs_collect_ident_refs_expr(end, out); }
        _ => {}
    }
}

fn rs_collect_ident_refs_stmts(stmts: &[Stmt], out: &mut std::collections::HashSet<String>) {
    for st in stmts {
        match st {
            Stmt::Let { value, .. } | Stmt::Expr(value) | Stmt::Throw(value) => rs_collect_ident_refs_expr(value, out),
            Stmt::Assign { target, value } => {
                match target {
                    AssignTarget::Ident(n) => { out.insert(n.clone()); }
                    AssignTarget::Index { target, index } => { rs_collect_ident_refs_expr(target, out); rs_collect_ident_refs_expr(index, out); }
                    AssignTarget::Member { target, .. } => rs_collect_ident_refs_expr(target, out),
                }
                rs_collect_ident_refs_expr(value, out);
            }
            Stmt::Return(Some(e)) => rs_collect_ident_refs_expr(e, out),
            Stmt::If { cond, then_branch, else_branch } => {
                rs_collect_ident_refs_expr(cond, out);
                rs_collect_ident_refs_stmts(then_branch, out);
                if let Some(eb) = else_branch { rs_collect_ident_refs_stmts(eb, out); }
            }
            Stmt::While { cond, body } => { rs_collect_ident_refs_expr(cond, out); rs_collect_ident_refs_stmts(body, out); }
            Stmt::For { iterable, body, .. } => { rs_collect_ident_refs_expr(iterable, out); rs_collect_ident_refs_stmts(body, out); }
            Stmt::TryCatch { try_body, catch_body, .. } => { rs_collect_ident_refs_stmts(try_body, out); rs_collect_ident_refs_stmts(catch_body, out); }
            Stmt::FnDecl { body, .. } | Stmt::Test { body, .. } => rs_collect_ident_refs_stmts(body, out),
            Stmt::Defer(inner) => rs_collect_ident_refs_stmts(&[(**inner).clone()], out),
            _ => {}
        }
    }
}

fn rs_stmts_have_return(stmts: &[Stmt]) -> bool {
    for st in stmts {
        match st {
            Stmt::Return(_) => return true,
            Stmt::If { then_branch, else_branch, .. } => {
                if rs_stmts_have_return(then_branch) || else_branch.as_deref().map(rs_stmts_have_return).unwrap_or(false) {
                    return true;
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                if rs_stmts_have_return(body) { return true; }
            }
            Stmt::TryCatch { try_body, catch_body, .. } => {
                if rs_stmts_have_return(try_body) || rs_stmts_have_return(catch_body) { return true; }
            }
            _ => {}
        }
    }
    false
}

/// Emits 100% pure, human-written Rust (`f64`, `&'static str`, `bool`, `println!`)
/// with ZERO `SVal` runtime header whenever a program's types are statically resolvable!
fn try_emit_human_rust(program: &Program) -> Option<String> {
    use crate::typecheck::{analyze_program, InferredType};
    let report = analyze_program(program);
    let mut fn_sigs: std::collections::BTreeMap<String, (Vec<InferredType>, InferredType)> =
        std::collections::BTreeMap::new();

    for stmt in &program.statements {
        match stmt {
            Stmt::ClassDecl { .. } | Stmt::TryCatch { .. } | Stmt::Throw(_) => return None,
            Stmt::FnDecl { name, params, body } => {
                let mut local_env = report.symbols.clone();
                let mut p_types = Vec::new();
                for (p, def) in params {
                    let guessed = guess_param_type_in_body(p, def.as_ref(), body)?;
                    local_env.insert(p.clone(), guessed.clone());
                    p_types.push(guessed);
                }
                let ret_ty = infer_fn_return_type(body, &local_env)?;
                fn_sigs.insert(name.clone(), (p_types, ret_ty));
            }
            _ => {}
        }
    }

    let mut env: std::collections::BTreeMap<String, InferredType> = std::collections::BTreeMap::new();
    for (k, (_, r)) in &fn_sigs {
        env.insert(
            k.clone(),
            InferredType::Func {
                params: 0,
                ret: Box::new(r.clone()),
            },
        );
    }

    let mut fns_out = String::new();
    let mut main_out = String::new();
    let mut declared = std::collections::HashSet::new();

    for stmt in &program.statements {
        if let Stmt::FnDecl { name, params, body } = stmt {
            let (p_types, ret_ty) = fn_sigs.get(name)?;
            let mut local_env = env.clone();
            let mut local_decl = std::collections::HashSet::new();
            let mut sig_parts = Vec::new();
            for ((p_name, _), p_ty) in params.iter().zip(p_types.iter()) {
                local_env.insert(p_name.clone(), p_ty.clone());
                local_decl.insert(p_name.clone());
                sig_parts.push(format!("{}: {}", p_name, rs_native_type(p_ty)?));
            }
            let ret_str = if *ret_ty == InferredType::Null {
                String::new()
            } else {
                format!(" -> {}", rs_native_type(ret_ty)?)
            };
            fns_out.push_str(&format!("fn {}({}){} {{\n", name, sig_parts.join(", "), ret_str));
            for st in body {
                fns_out.push_str(&human_rs_stmt(st, 1, &mut local_env, &mut local_decl, &fn_sigs)?);
            }
            fns_out.push_str("}\n\n");
        } else {
            main_out.push_str(&human_rs_stmt(stmt, 1, &mut env, &mut declared, &fn_sigs)?);
        }
    }

    let mut out = String::from("#![allow(unused_variables,unused_mut,unused_parens,dead_code,non_snake_case)]\n\n");
    out.push_str(&fns_out);
    out.push_str("fn main() {\n");
    out.push_str(&main_out);
    out.push_str("}\n");
    Some(out)
}

fn rs_native_type(ty: &crate::typecheck::InferredType) -> Option<&'static str> {
    use crate::typecheck::InferredType;
    match ty {
        InferredType::Num => Some("f64"),
        InferredType::Str => Some("&'static str"),
        InferredType::Bool => Some("bool"),
        InferredType::Null => Some("()"),
        _ => None,
    }
}

fn human_rs_stmt(
    stmt: &Stmt,
    lvl: usize,
    env: &mut std::collections::BTreeMap<String, crate::typecheck::InferredType>,
    declared: &mut std::collections::HashSet<String>,
    fn_sigs: &std::collections::BTreeMap<
        String,
        (Vec<crate::typecheck::InferredType>, crate::typecheck::InferredType),
    >,
) -> Option<String> {
    use crate::typecheck::{infer_expr, InferredType};
    let pad = indent(lvl);
    match stmt {
        Stmt::Let { name, value, .. } => {
            let ty = infer_expr(value, env);
            let _ = rs_native_type(&ty)?;
            let val_s = human_rs_expr(value, env, fn_sigs)?;
            env.insert(name.clone(), ty);
            declared.insert(name.clone());
            Some(format!("{}let mut {} = {};\n", pad, name, val_s))
        }
        Stmt::Assign {
            target: AssignTarget::Ident(name),
            value,
        } => {
            let ty = infer_expr(value, env);
            let _ = rs_native_type(&ty)?;
            let val_s = human_rs_expr(value, env, fn_sigs)?;
            let var_name = if fn_sigs.contains_key(name) {
                format!("{}_val", name)
            } else {
                name.clone()
            };
            env.insert(name.clone(), ty.clone());
            env.insert(var_name.clone(), ty);
            if !declared.contains(&var_name) {
                declared.insert(var_name.clone());
                Some(format!("{}let mut {} = {};\n", pad, var_name, val_s))
            } else {
                Some(format!("{}{} = {};\n", pad, var_name, val_s))
            }
        }
        Stmt::Return(Some(e)) => Some(format!("{}return {};\n", pad, human_rs_expr(e, env, fn_sigs)?)),
        Stmt::Return(None) => Some(format!("{}return;\n", pad)),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let mut s = format!("{}if {} {{\n", pad, human_rs_expr(cond, env, fn_sigs)?);
            for st in then_branch {
                s.push_str(&human_rs_stmt(st, lvl + 1, env, declared, fn_sigs)?);
            }
            if let Some(eb) = else_branch {
                s.push_str(&format!("{}}} else {{\n", pad));
                for st in eb {
                    s.push_str(&human_rs_stmt(st, lvl + 1, env, declared, fn_sigs)?);
                }
            }
            s.push_str(&format!("{}}}\n", pad));
            Some(s)
        }
        Stmt::While { cond, body } => {
            let mut s = format!("{}while {} {{\n", pad, human_rs_expr(cond, env, fn_sigs)?);
            for st in body {
                s.push_str(&human_rs_stmt(st, lvl + 1, env, declared, fn_sigs)?);
            }
            s.push_str(&format!("{}}}\n", pad));
            Some(s)
        }
        Stmt::For {
            var,
            iterable: Expr::Range { start, end },
            body,
        } => {
            let s_val = human_rs_expr(start, env, fn_sigs)?;
            let e_val = human_rs_expr(end, env, fn_sigs)?;
            env.insert(var.clone(), InferredType::Num);
            declared.insert(var.clone());
            let mut s = format!(
                "{}for __i in ({} as i64)..({} as i64) {{\n{}    let mut {} = __i as f64;\n",
                pad, s_val, e_val, pad, var
            );
            for st in body {
                s.push_str(&human_rs_stmt(st, lvl + 1, env, declared, fn_sigs)?);
            }
            s.push_str(&format!("{}}}\n", pad));
            Some(s)
        }
        Stmt::Expr(Expr::Call { callee, args }) => {
            if let Expr::Ident(name) = &**callee {
                if name == "print" || name == "println" {
                    let mut fmt_parts = Vec::new();
                    let mut rs_args = Vec::new();
                    for a in args {
                        fmt_parts.push("{}");
                        rs_args.push(human_rs_expr(a, env, fn_sigs)?);
                    }
                    if rs_args.is_empty() {
                        return Some(format!("{}println!();\n", pad));
                    } else {
                        return Some(format!(
                            "{}println!(\"{}\", {});\n",
                            pad,
                            fmt_parts.join(" "),
                            rs_args.join(", ")
                        ));
                    }
                }
            }
            Some(format!(
                "{}{};\n",
                pad,
                human_rs_expr(
                    &Expr::Call {
                        callee: callee.clone(),
                        args: args.clone()
                    },
                    env,
                    fn_sigs
                )?
            ))
        }
        _ => None,
    }
}

fn human_rs_expr(
    expr: &Expr,
    env: &std::collections::BTreeMap<String, crate::typecheck::InferredType>,
    fn_sigs: &std::collections::BTreeMap<
        String,
        (Vec<crate::typecheck::InferredType>, crate::typecheck::InferredType),
    >,
) -> Option<String> {
    use crate::typecheck::{infer_expr, InferredType};
    match expr {
        Expr::Number(n) => Some(if n.fract() == 0.0 {
            format!("{:.1}", n)
        } else {
            format!("{}", n)
        }),
        Expr::String(s) => Some(format!("\"{}\"", escape_str(s))),
        Expr::Bool(b) => Some(b.to_string()),
        Expr::Ident(n) => {
            let actual = if env.contains_key(&format!("{}_val", n)) {
                format!("{}_val", n)
            } else {
                n.clone()
            };
            if env.contains_key(n) || fn_sigs.contains_key(n) {
                Some(actual)
            } else {
                Some(format!("\"{}\"", escape_str(n)))
            }
        }
        Expr::Unary { op, expr: inner } => {
            let e = human_rs_expr(inner, env, fn_sigs)?;
            match op {
                UnaryOp::Neg => Some(format!("(-{})", e)),
                UnaryOp::Not => Some(format!("(!{})", e)),
                _ => None,
            }
        }
        Expr::Binary { left, op, right } => {
            let lt = infer_expr(left, env);
            let rt = infer_expr(right, env);
            if matches!(op, BinaryOp::Add) && (lt == InferredType::Str || rt == InferredType::Str) {
                return None;
            }
            let l = human_rs_expr(left, env, fn_sigs)?;
            let r = human_rs_expr(right, env, fn_sigs)?;
            let op_s = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
                BinaryOp::Mod => "%",
                BinaryOp::Eq => "==",
                BinaryOp::Neq => "!=",
                BinaryOp::Lt => "<",
                BinaryOp::Gt => ">",
                BinaryOp::Lte => "<=",
                BinaryOp::Gte => ">=",
                BinaryOp::And => "&&",
                BinaryOp::Or => "||",
                _ => return None,
            };
            Some(format!("({} {} {})", l, op_s, r))
        }
        Expr::Ternary {
            cond,
            then_expr,
            else_expr,
        } => {
            let c = human_rs_expr(cond, env, fn_sigs)?;
            let t = human_rs_expr(then_expr, env, fn_sigs)?;
            let e = human_rs_expr(else_expr, env, fn_sigs)?;
            Some(format!("(if {} {{ {} }} else {{ {} }})", c, t, e))
        }
        Expr::Call { callee, args } => {
            if let Expr::Ident(name) = &**callee {
                if fn_sigs.contains_key(name) {
                    let mut a_strs = Vec::new();
                    for a in args {
                        a_strs.push(human_rs_expr(a, env, fn_sigs)?);
                    }
                    return Some(format!("{}({})", name, a_strs.join(", ")));
                }
            }
            None
        }
        _ => None,
    }
}

fn rs_builtins_preamble(body_code: &str) -> String {
    let mut out = String::new();
    for (token, decl) in [
        ("PI.clone()", "    let PI = SVal::Num(std::f64::consts::PI);\n"),
        ("E.clone()", "    let E = SVal::Num(std::f64::consts::E);\n"),
        ("print.clone()", "    let print = SVal::Func(Rc::new(|args: Vec<SVal>| { println!(\"{}\", args.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(\" \")); SVal::Null }));\n"),
        ("len.clone()", "    let len = SVal::Func(Rc::new(|args: Vec<SVal>| match args.first() { Some(SVal::Arr(a)) => SVal::Num(a.borrow().len() as f64), Some(SVal::Str(s)) => SVal::Num(s.chars().count() as f64), Some(SVal::Map(m)) => SVal::Num(m.borrow().len() as f64), _ => SVal::Num(0.0) }));\n"),
        ("push.clone()", "    let push = SVal::Func(Rc::new(|args: Vec<SVal>| { if let (Some(SVal::Arr(a)), Some(v)) = (args.get(0), args.get(1)) { a.borrow_mut().push(v.clone()); return SVal::Arr(a.clone()); } SVal::Null }));\n"),
        ("pop.clone()", "    let pop = SVal::Func(Rc::new(|args: Vec<SVal>| { if let Some(SVal::Arr(a)) = args.get(0) { return a.borrow_mut().pop().unwrap_or(SVal::Null); } SVal::Null }));\n"),
        ("keys.clone()", "    let keys = SVal::Func(Rc::new(|args: Vec<SVal>| { if let Some(SVal::Map(m)) = args.get(0) { let ks = m.borrow().keys().map(|k| SVal::Str(k.clone())).collect(); return SVal::Arr(Rc::new(RefCell::new(ks))); } SVal::Null }));\n"),
        ("values.clone()", "    let values = SVal::Func(Rc::new(|args: Vec<SVal>| { if let Some(SVal::Map(m)) = args.get(0) { let vs = m.borrow().values().cloned().collect(); return SVal::Arr(Rc::new(RefCell::new(vs))); } SVal::Null }));\n"),
        ("has_key.clone()", "    let has_key = SVal::Func(Rc::new(|args: Vec<SVal>| { if let (Some(SVal::Map(m)), Some(k)) = (args.get(0), args.get(1)) { return SVal::Bool(m.borrow().contains_key(&k.to_string())); } SVal::Bool(false) }));\n"),
        ("delete_key.clone()", "    let delete_key = SVal::Func(Rc::new(|args: Vec<SVal>| { if let (Some(SVal::Map(m)), Some(k)) = (args.get(0), args.get(1)) { return SVal::Bool(m.borrow_mut().remove(&k.to_string()).is_some()); } SVal::Bool(false) }));\n"),
        ("concat.clone()", "    let concat = SVal::Func(Rc::new(|args: Vec<SVal>| { if let (Some(SVal::Arr(a)), Some(SVal::Arr(b))) = (args.get(0), args.get(1)) { let mut out = a.borrow().clone(); out.extend(b.borrow().iter().cloned()); return SVal::Arr(Rc::new(RefCell::new(out))); } SVal::Arr(Rc::new(RefCell::new(vec![]))) }));\n"),
        ("str_fn.clone()", "    let str_fn = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Str(args.first().cloned().unwrap_or(SVal::Null).to_string())));\n"),
        ("num_fn.clone()", "    let num_fn = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.first().cloned().unwrap_or(SVal::Null).as_num())));\n"),
        ("int_fn.clone()", "    let int_fn = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.first().cloned().unwrap_or(SVal::Null).as_num().trunc())));\n"),
        ("bool_fn.clone()", "    let bool_fn = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Bool(args.first().map(|a| a.is_truthy()).unwrap_or(false))));\n"),
        ("type_fn.clone()", "    let type_fn = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Str(match args.first() { Some(SVal::Null) | None => \"null\", Some(SVal::Bool(_)) => \"boolean\", Some(SVal::Num(_)) => \"number\", Some(SVal::Str(_)) => \"string\", Some(SVal::Arr(_)) => \"array\", Some(SVal::Map(_)) => \"map\", Some(SVal::Func(_)) => \"function\" }.to_string())));\n"),
        ("range.clone()", "    let range = SVal::Func(Rc::new(|args: Vec<SVal>| { let s = args.get(0).map(|a| a.as_num() as i64).unwrap_or(0); let e = args.get(1).map(|a| a.as_num() as i64).unwrap_or(0); let step = args.get(2).map(|a| a.as_num() as i64).unwrap_or(1); let mut v = Vec::new(); let mut cur = s; if step > 0 { while cur < e { v.push(SVal::Num(cur as f64)); cur += step; } } else if step < 0 { while cur > e { v.push(SVal::Num(cur as f64)); cur += step; } } SVal::Arr(Rc::new(RefCell::new(v))) }));\n"),
        ("map.clone()", "    let map = SVal::Func(Rc::new(|args: Vec<SVal>| { if let (Some(SVal::Arr(a)), Some(f)) = (args.get(0), args.get(1)) { let out = a.borrow().iter().map(|x| f.call(vec![x.clone()])).collect(); return SVal::Arr(Rc::new(RefCell::new(out))); } SVal::Null }));\n"),
        ("filter.clone()", "    let filter = SVal::Func(Rc::new(|args: Vec<SVal>| { if let (Some(SVal::Arr(a)), Some(f)) = (args.get(0), args.get(1)) { let out = a.borrow().iter().filter(|x| f.call(vec![(*x).clone()]).is_truthy()).cloned().collect(); return SVal::Arr(Rc::new(RefCell::new(out))); } SVal::Null }));\n"),
        ("reduce.clone()", "    let reduce = SVal::Func(Rc::new(|args: Vec<SVal>| { if let (Some(SVal::Arr(a)), Some(f)) = (args.get(0), args.get(1)) { let items = a.borrow().clone(); let mut it = items.into_iter(); let mut acc = args.get(2).cloned().or_else(|| it.next()).unwrap_or(SVal::Null); for x in it { acc = f.call(vec![acc, x]); } return acc; } SVal::Null }));\n"),
        ("find.clone()", "    let find = SVal::Func(Rc::new(|args: Vec<SVal>| { if let (Some(SVal::Arr(a)), Some(f)) = (args.get(0), args.get(1)) { for x in a.borrow().iter() { if f.call(vec![x.clone()]).is_truthy() { return x.clone(); } } } SVal::Null }));\n"),
        ("any.clone()", "    let any = SVal::Func(Rc::new(|args: Vec<SVal>| { if let (Some(SVal::Arr(a)), Some(f)) = (args.get(0), args.get(1)) { for x in a.borrow().iter() { if f.call(vec![x.clone()]).is_truthy() { return SVal::Bool(true); } } } SVal::Bool(false) }));\n"),
        ("all.clone()", "    let all = SVal::Func(Rc::new(|args: Vec<SVal>| { if let (Some(SVal::Arr(a)), Some(f)) = (args.get(0), args.get(1)) { for x in a.borrow().iter() { if !f.call(vec![x.clone()]).is_truthy() { return SVal::Bool(false); } } return SVal::Bool(true); } SVal::Bool(false) }));\n"),
        ("sum.clone()", "    let sum = SVal::Func(Rc::new(|args: Vec<SVal>| { let mut s = 0.0; if let Some(SVal::Arr(a)) = args.get(0) { for x in a.borrow().iter() { if let SVal::Num(n) = x { s += *n; } } } SVal::Num(s) }));\n"),
        ("avg.clone()", "    let avg = SVal::Func(Rc::new(|args: Vec<SVal>| { if let Some(SVal::Arr(a)) = args.get(0) { let b = a.borrow(); if b.is_empty() { return SVal::Num(0.0); } let mut s = 0.0; for x in b.iter() { if let SVal::Num(n) = x { s += *n; } } return SVal::Num(s / b.len() as f64); } SVal::Num(0.0) }));\n"),
        ("unique.clone()", "    let unique = SVal::Func(Rc::new(|args: Vec<SVal>| { let mut out: Vec<SVal> = Vec::new(); if let Some(SVal::Arr(a)) = args.get(0) { for x in a.borrow().iter() { if !out.contains(x) { out.push(x.clone()); } } } SVal::Arr(Rc::new(RefCell::new(out))) }));\n"),
        ("flat.clone()", "    let flat = SVal::Func(Rc::new(|args: Vec<SVal>| { let mut out: Vec<SVal> = Vec::new(); if let Some(SVal::Arr(a)) = args.get(0) { for x in a.borrow().iter() { if let SVal::Arr(inner) = x { out.extend(inner.borrow().iter().cloned()); } else { out.push(x.clone()); } } } SVal::Arr(Rc::new(RefCell::new(out))) }));\n"),
        ("zip.clone()", "    let zip = SVal::Func(Rc::new(|args: Vec<SVal>| { let mut out: Vec<SVal> = Vec::new(); if let (Some(SVal::Arr(a)), Some(SVal::Arr(b))) = (args.get(0), args.get(1)) { for (x, y) in a.borrow().iter().zip(b.borrow().iter()) { out.push(SVal::Arr(Rc::new(RefCell::new(vec![x.clone(), y.clone()])))); } } SVal::Arr(Rc::new(RefCell::new(out))) }));\n"),
        ("enumerate.clone()", "    let enumerate = SVal::Func(Rc::new(|args: Vec<SVal>| { let mut out: Vec<SVal> = Vec::new(); if let Some(SVal::Arr(a)) = args.get(0) { for (i, x) in a.borrow().iter().enumerate() { out.push(SVal::Arr(Rc::new(RefCell::new(vec![SVal::Num(i as f64), x.clone()])))); } } SVal::Arr(Rc::new(RefCell::new(out))) }));\n"),
        ("upper.clone()", "    let upper = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Str(args.first().map(|a| a.to_string().to_uppercase()).unwrap_or_default())));\n"),
        ("lower.clone()", "    let lower = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Str(args.first().map(|a| a.to_string().to_lowercase()).unwrap_or_default())));\n"),
        ("trim.clone()", "    let trim = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Str(args.first().map(|a| a.to_string().trim().to_string()).unwrap_or_default())));\n"),
        ("contains.clone()", "    let contains = SVal::Func(Rc::new(|args: Vec<SVal>| match args.get(0) { Some(SVal::Map(m)) => { let k = args.get(1).map(|a| a.to_string()).unwrap_or_default(); SVal::Bool(m.borrow().contains_key(&k)) } Some(SVal::Arr(a)) => { let target = args.get(1).cloned().unwrap_or(SVal::Null); SVal::Bool(a.borrow().iter().any(|x| x.to_string() == target.to_string())) } _ => { let h = args.get(0).map(|a| a.to_string()).unwrap_or_default(); let n = args.get(1).map(|a| a.to_string()).unwrap_or_default(); SVal::Bool(h.contains(&n)) } }));\n"),
        ("index_of.clone()", "    let index_of = SVal::Func(Rc::new(|args: Vec<SVal>| match (args.get(0), args.get(1)) { (Some(SVal::Str(s)), Some(SVal::Str(sub))) => SVal::Num(s.find(sub.as_str()).map(|bi| s[..bi].chars().count() as f64).unwrap_or(-1.0)), (Some(SVal::Arr(a)), Some(n)) => SVal::Num(a.borrow().iter().position(|x| x == n).map(|i| i as f64).unwrap_or(-1.0)), _ => SVal::Num(-1.0) }));\n"),
        ("starts_with.clone()", "    let starts_with = SVal::Func(Rc::new(|args: Vec<SVal>| { let h = args.get(0).map(|a| a.to_string()).unwrap_or_default(); let p = args.get(1).map(|a| a.to_string()).unwrap_or_default(); SVal::Bool(h.starts_with(&p)) }));\n"),
        ("ends_with.clone()", "    let ends_with = SVal::Func(Rc::new(|args: Vec<SVal>| { let h = args.get(0).map(|a| a.to_string()).unwrap_or_default(); let p = args.get(1).map(|a| a.to_string()).unwrap_or_default(); SVal::Bool(h.ends_with(&p)) }));\n"),
        ("repeat.clone()", "    let repeat = SVal::Func(Rc::new(|args: Vec<SVal>| { let s = args.get(0).map(|a| a.to_string()).unwrap_or_default(); let n = args.get(1).map(|a| (a.as_num() as isize).max(0) as usize).unwrap_or(0); SVal::Str(s.repeat(n)) }));\n"),
        ("pad_start.clone()", "    let pad_start = SVal::Func(Rc::new(|args: Vec<SVal>| { let s = args.get(0).map(|a| a.to_string()).unwrap_or_default(); let w = args.get(1).map(|a| (a.as_num() as isize).max(0) as usize).unwrap_or(0); let p = args.get(2).map(|a| a.to_string()).unwrap_or_else(|| \" \".to_string()); let cur = s.chars().count(); if cur >= w || p.is_empty() { return SVal::Str(s); } let pc: Vec<char> = p.chars().collect(); let mut out = String::new(); for i in 0..(w - cur) { out.push(pc[i % pc.len()]); } out.push_str(&s); SVal::Str(out) }));\n"),
        ("pad_end.clone()", "    let pad_end = SVal::Func(Rc::new(|args: Vec<SVal>| { let s = args.get(0).map(|a| a.to_string()).unwrap_or_default(); let w = args.get(1).map(|a| (a.as_num() as isize).max(0) as usize).unwrap_or(0); let p = args.get(2).map(|a| a.to_string()).unwrap_or_else(|| \" \".to_string()); let cur = s.chars().count(); if cur >= w || p.is_empty() { return SVal::Str(s); } let pc: Vec<char> = p.chars().collect(); let mut out = s; for i in 0..(w - cur) { out.push(pc[i % pc.len()]); } SVal::Str(out) }));\n"),
        ("char_code.clone()", "    let char_code = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.get(0).map(|a| a.to_string().chars().next().map(|c| c as u32 as f64).unwrap_or(0.0)).unwrap_or(0.0))));\n"),
        ("from_char_code.clone()", "    let from_char_code = SVal::Func(Rc::new(|args: Vec<SVal>| { let n = args.get(0).map(|a| (a.as_num() as i64).max(0) as u32).unwrap_or(0); SVal::Str(char::from_u32(n).unwrap_or('\\0').to_string()) }));\n"),
        ("replace.clone()", "    let replace = SVal::Func(Rc::new(|args: Vec<SVal>| { let s = args.get(0).map(|a| a.to_string()).unwrap_or_default(); let a = args.get(1).map(|x| x.to_string()).unwrap_or_default(); let b = args.get(2).map(|x| x.to_string()).unwrap_or_default(); SVal::Str(s.replace(&a, &b)) }));\n"),
        ("reverse.clone()", "    let reverse = SVal::Func(Rc::new(|args: Vec<SVal>| match args.get(0) { Some(SVal::Arr(a)) => { let mut v = a.borrow().clone(); v.reverse(); SVal::Arr(Rc::new(RefCell::new(v))) } Some(SVal::Str(s)) => SVal::Str(s.chars().rev().collect()), _ => SVal::Null }));\n"),
        ("slice.clone()", "    let slice = SVal::Func(Rc::new(|args: Vec<SVal>| { let s = args.get(1).map(|a| a.as_num() as usize).unwrap_or(0); match args.get(0) { Some(SVal::Arr(a)) => { let b = a.borrow(); let e = args.get(2).map(|x| (x.as_num() as usize).min(b.len())).unwrap_or(b.len()); let s = s.min(e); SVal::Arr(Rc::new(RefCell::new(b[s..e].to_vec()))) } Some(SVal::Str(st)) => { let cs: Vec<char> = st.chars().collect(); let e = args.get(2).map(|x| (x.as_num() as usize).min(cs.len())).unwrap_or(cs.len()); let s = s.min(e); SVal::Str(cs[s..e].iter().collect()) } _ => SVal::Null } }));\n"),
        ("sort.clone()", "    let sort = SVal::Func(Rc::new(|args: Vec<SVal>| { if let Some(SVal::Arr(a)) = args.get(0) { let mut v = a.borrow().clone(); v.sort_by(|x, y| x.to_string().cmp(&y.to_string())); return SVal::Arr(Rc::new(RefCell::new(v))); } SVal::Null }));\n"),
        ("pow.clone()", "    let pow = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.get(0).map(|a| a.as_num()).unwrap_or(0.0).powf(args.get(1).map(|b| b.as_num()).unwrap_or(0.0)))));\n"),
        ("clamp.clone()", "    let clamp = SVal::Func(Rc::new(|args: Vec<SVal>| { let x = args.get(0).map(|a| a.as_num()).unwrap_or(0.0); let lo = args.get(1).map(|a| a.as_num()).unwrap_or(0.0); let hi = args.get(2).map(|a| a.as_num()).unwrap_or(0.0); SVal::Num(x.max(lo).min(hi)) }));\n"),
        ("sign.clone()", "    let sign = SVal::Func(Rc::new(|args: Vec<SVal>| { let x = args.get(0).map(|a| a.as_num()).unwrap_or(0.0); SVal::Num(if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }) }));\n"),
        ("atan2.clone()", "    let atan2 = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.get(0).map(|a| a.as_num()).unwrap_or(0.0).atan2(args.get(1).map(|b| b.as_num()).unwrap_or(0.0)))));\n"),
        ("log.clone()", "    let log = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.first().map(|a| a.as_num().ln()).unwrap_or(0.0))));\n"),
        ("exp.clone()", "    let exp = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.first().map(|a| a.as_num().exp()).unwrap_or(0.0))));\n"),
        ("read_file.clone()", "    let read_file = SVal::Func(Rc::new(|args: Vec<SVal>| { let p = args.get(0).map(|a| a.to_string()).unwrap_or_default(); SVal::Str(std::fs::read_to_string(&p).unwrap_or_default()) }));\n"),
        ("write_file.clone()", "    let write_file = SVal::Func(Rc::new(|args: Vec<SVal>| { let p = args.get(0).map(|a| a.to_string()).unwrap_or_default(); let c = args.get(1).map(|a| a.to_string()).unwrap_or_default(); SVal::Bool(std::fs::write(&p, c).is_ok()) }));\n"),
        ("append_file.clone()", "    let append_file = SVal::Func(Rc::new(|args: Vec<SVal>| { use std::io::Write; let p = args.get(0).map(|a| a.to_string()).unwrap_or_default(); let c = args.get(1).map(|a| a.to_string()).unwrap_or_default(); SVal::Bool(std::fs::OpenOptions::new().create(true).append(true).open(&p).and_then(|mut f| f.write_all(c.as_bytes())).is_ok()) }));\n"),
        ("remove_file.clone()", "    let remove_file = SVal::Func(Rc::new(|args: Vec<SVal>| { let p = args.get(0).map(|a| a.to_string()).unwrap_or_default(); SVal::Bool(std::fs::remove_file(&p).is_ok()) }));\n"),
        ("file_exists.clone()", "    let file_exists = SVal::Func(Rc::new(|args: Vec<SVal>| { let p = args.get(0).map(|a| a.to_string()).unwrap_or_default(); SVal::Bool(std::path::Path::new(&p).exists()) }));\n"),
        ("env.clone()", "    let env = SVal::Func(Rc::new(|args: Vec<SVal>| { let k = args.get(0).map(|a| a.to_string()).unwrap_or_default(); match std::env::var(&k) { Ok(v) => SVal::Str(v), Err(_) => SVal::Null } }));\n"),
        ("time_ms.clone()", "    let time_ms = SVal::Func(Rc::new(|_args: Vec<SVal>| { let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0); SVal::Num(ms) }));\n"),
        ("clock.clone()", "    let clock = SVal::Func(Rc::new(|_args: Vec<SVal>| { let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0); SVal::Num(s) }));\n"),
        ("hash.clone()", "    let hash = SVal::Func(Rc::new(|args: Vec<SVal>| { let s = args.first().map(|a| a.to_string()).unwrap_or_default(); let mut h: u64 = 0xcbf29ce484222325; for b in s.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x0100000001b3); } SVal::Str(format!(\"{:016x}\", h)) }));\n"),
        ("json_stringify.clone()", "    fn __rs_json_str(v: &SVal) -> String { match v { SVal::Null => \"null\".into(), SVal::Bool(b) => b.to_string(), SVal::Num(n) => if n.fract() == 0.0 && n.abs() < 1e15 { format!(\"{}\", *n as i64) } else { format!(\"{}\", n) }, SVal::Str(s) => format!(\"\\\"{}\\\"\", s.replace('\\\\', \"\\\\\\\\\").replace('\"', \"\\\\\\\"\").replace('\\n', \"\\\\n\").replace('\\r', \"\\\\r\").replace('\\t', \"\\\\t\")), SVal::Arr(a) => format!(\"[{}]\", a.borrow().iter().map(__rs_json_str).collect::<Vec<_>>().join(\",\")), SVal::Map(m) => format!(\"{{{}}}\", m.borrow().iter().map(|(k, val)| format!(\"\\\"{}\\\": {}\", k.replace('\\\\', \"\\\\\\\\\").replace('\"', \"\\\\\\\"\"), __rs_json_str(val)).replace(\"\\\": \", \"\\\":\")).collect::<Vec<_>>().join(\",\")), SVal::Func(_) => \"null\".into() } }\n    let json_stringify = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Str(__rs_json_str(args.first().unwrap_or(&SVal::Null)))));\n"),
        ("json_parse.clone()", "    fn __rs_jp(cs: &[char], i: &mut usize) -> SVal { while *i < cs.len() && cs[*i].is_whitespace() { *i += 1; } if *i >= cs.len() { return SVal::Null; } match cs[*i] { '\"' => { *i += 1; let mut s = String::new(); while *i < cs.len() { let c = cs[*i]; *i += 1; if c == '\"' { break; } if c == '\\\\' && *i < cs.len() { let e = cs[*i]; *i += 1; match e { 'n' => s.push('\\n'), 'r' => s.push('\\r'), 't' => s.push('\\t'), o => s.push(o) } } else { s.push(c); } } SVal::Str(s) } '[' => { *i += 1; let mut a = Vec::new(); loop { while *i < cs.len() && cs[*i].is_whitespace() { *i += 1; } if *i >= cs.len() || cs[*i] == ']' { *i += 1; break; } a.push(__rs_jp(cs, i)); while *i < cs.len() && cs[*i].is_whitespace() { *i += 1; } if *i < cs.len() && cs[*i] == ',' { *i += 1; } } SVal::Arr(Rc::new(RefCell::new(a))) } '{' => { *i += 1; let mut m = BTreeMap::new(); loop { while *i < cs.len() && cs[*i].is_whitespace() { *i += 1; } if *i >= cs.len() || cs[*i] == '}' { *i += 1; break; } let k = __rs_jp(cs, i).to_string(); while *i < cs.len() && (cs[*i].is_whitespace() || cs[*i] == ':') { *i += 1; } let v = __rs_jp(cs, i); m.insert(k, v); while *i < cs.len() && cs[*i].is_whitespace() { *i += 1; } if *i < cs.len() && cs[*i] == ',' { *i += 1; } } SVal::Map(Rc::new(RefCell::new(m))) } 't' => { *i += 4; SVal::Bool(true) } 'f' => { *i += 5; SVal::Bool(false) } 'n' => { *i += 4; SVal::Null } _ => { let st = *i; while *i < cs.len() && (cs[*i].is_ascii_digit() || matches!(cs[*i], '-' | '+' | '.' | 'e' | 'E')) { *i += 1; } let ns: String = cs[st..*i].iter().collect(); SVal::Num(ns.parse().unwrap_or(0.0)) } } }\n    let json_parse = SVal::Func(Rc::new(|args: Vec<SVal>| { let s = args.first().map(|a| a.to_string()).unwrap_or_default(); let cs: Vec<char> = s.chars().collect(); let mut i = 0; __rs_jp(&cs, &mut i) }));\n"),
        ("split.clone()", "    let split = SVal::Func(Rc::new(|args: Vec<SVal>| { let s = args.get(0).map(|a| a.to_string()).unwrap_or_default(); let sep = args.get(1).map(|a| a.to_string()).unwrap_or_default(); let parts: Vec<SVal> = if sep.is_empty() { s.chars().map(|c| SVal::Str(c.to_string())).collect() } else { s.split(&sep).map(|p| SVal::Str(p.to_string())).collect() }; SVal::Arr(Rc::new(RefCell::new(parts))) }));\n"),
        ("join.clone()", "    let join = SVal::Func(Rc::new(|args: Vec<SVal>| { if let Some(SVal::Arr(a)) = args.get(0) { let sep = args.get(1).map(|s| s.to_string()).unwrap_or_default(); return SVal::Str(a.borrow().iter().map(|x| x.to_string()).collect::<Vec<_>>().join(&sep)); } SVal::Str(String::new()) }));\n"),
        ("abs.clone()", "    let abs = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.first().map(|a| a.as_num().abs()).unwrap_or(0.0))));\n"),
        ("sqrt.clone()", "    let sqrt = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.first().map(|a| a.as_num().sqrt()).unwrap_or(0.0))));\n"),
        ("sin.clone()", "    let sin = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.first().map(|a| a.as_num().sin()).unwrap_or(0.0))));\n"),
        ("cos.clone()", "    let cos = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.first().map(|a| a.as_num().cos()).unwrap_or(0.0))));\n"),
        ("tan.clone()", "    let tan = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.first().map(|a| a.as_num().tan()).unwrap_or(0.0))));\n"),
        ("floor.clone()", "    let floor = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.first().map(|a| a.as_num().floor()).unwrap_or(0.0))));\n"),
        ("ceil.clone()", "    let ceil = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.first().map(|a| a.as_num().ceil()).unwrap_or(0.0))));\n"),
        ("round.clone()", "    let round = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.first().map(|a| a.as_num().round()).unwrap_or(0.0))));\n"),
        ("min.clone()", "    let min = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.get(0).map(|a| a.as_num()).unwrap_or(0.0).min(args.get(1).map(|b| b.as_num()).unwrap_or(0.0)))));\n"),
        ("max.clone()", "    let max = SVal::Func(Rc::new(|args: Vec<SVal>| SVal::Num(args.get(0).map(|a| a.as_num()).unwrap_or(0.0).max(args.get(1).map(|b| b.as_num()).unwrap_or(0.0)))));\n"),
        ("assert.clone()", "    let assert = SVal::Func(Rc::new(|args: Vec<SVal>| { if !args.first().map(|a| a.is_truthy()).unwrap_or(false) { panic!(\"{}\", args.get(1).map(|m| m.to_string()).unwrap_or_else(|| \"Assertion failed\".to_string())); } SVal::Bool(true) }));\n"),
    ] {
        if body_code.contains(token) {
            out.push_str(decl);
        }
    }
    out
}

fn rs_safe_ident(name: &str) -> String {
    match name {
        "as" | "async" | "await" | "box" | "crate" | "dyn" | "enum" | "extern" | "fn" | "impl"
        | "in" | "loop" | "macro" | "match" | "mod" | "move" | "mut" | "pub" | "ref" | "self"
        | "Self" | "static" | "struct" | "super" | "trait" | "type" | "unsafe" | "use"
        | "where" => format!("r#{}", name),
        _ => name.to_string(),
    }
}

fn rs_top_fn(
    name: &str,
    params: &[(String, Option<Expr>)],
    body: &[Stmt],
    top_fns: &std::collections::HashSet<String>,
) -> String {
    let param_sig: Vec<String> = params
        .iter()
        .map(|(p, _)| format!("mut {}: SVal", rs_safe_ident(p)))
        .collect();
    let mut body_str = String::new();
    for st in body {
        body_str.push_str(&rs_stmt(st, 1, top_fns));
    }
    let mut s = format!("fn {}({}) -> SVal {{\n", rs_safe_ident(name), param_sig.join(", "));
    s.push_str(&rs_builtins_preamble(&body_str));
    s.push_str(&body_str);
    s.push_str("    SVal::Null\n}\n");
    let wrap_args: Vec<String> = (0..params.len())
        .map(|i| format!("__a.get({}).cloned().unwrap_or(SVal::Null)", i))
        .collect();
    s.push_str(&format!(
        "#[allow(dead_code)] fn __steel_fn_{}(__a: Vec<SVal>) -> SVal {{ {}({}) }}\n\n",
        name,
        rs_safe_ident(name),
        wrap_args.join(", ")
    ));
    s
}

fn rs_try_stmt(stmt: &Stmt, lvl: usize, top_fns: &std::collections::HashSet<String>) -> String {
    let pad = indent(lvl);
    match stmt {
        Stmt::Return(val) => {
            let v = val.as_ref().map(|e| rs_expr(e, lvl, top_fns)).unwrap_or_else(|| "SVal::Null".to_string());
            format!("{}return Some({});\n", pad, v)
        }
        Stmt::If { cond, then_branch, else_branch } => {
            let mut s = format!("{}if ({}).is_truthy() {{\n", pad, rs_expr(cond, lvl, top_fns));
            for st in then_branch { s.push_str(&rs_try_stmt(st, lvl + 1, top_fns)); }
            if let Some(eb) = else_branch {
                s.push_str(&format!("{}}} else {{\n", pad));
                for st in eb { s.push_str(&rs_try_stmt(st, lvl + 1, top_fns)); }
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::While { cond, body } => {
            let mut s = format!("{}while ({}).is_truthy() {{\n", pad, rs_expr(cond, lvl, top_fns));
            for st in body { s.push_str(&rs_try_stmt(st, lvl + 1, top_fns)); }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::TryCatch { try_body, err_var, catch_body } => {
            let mut s = format!(
                "{}let __prev_hook = std::panic::take_hook();\n{}std::panic::set_hook(Box::new(|_| {{}}));\n{}let __res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Option<SVal> {{\n",
                pad, pad, pad
            );
            for st in try_body { s.push_str(&rs_try_stmt(st, lvl + 1, top_fns)); }
            s.push_str(&format!("{}    None\n{}}}));\n{}std::panic::set_hook(__prev_hook);\n", pad, pad, pad));
            s.push_str(&format!("{}if let Ok(Some(__ret_val)) = __res {{ return Some(__ret_val); }}\n", pad));
            s.push_str(&format!(
                "{}if let Err(__err) = __res {{\n{}    let __msg = __err.downcast_ref::<String>().cloned().or_else(|| __err.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| \"Caught error\".to_string());\n{}    let mut {} = SVal::Str(__msg);\n",
                pad, pad, pad, err_var
            ));
            for st in catch_body { s.push_str(&rs_try_stmt(st, lvl + 1, top_fns)); }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        other => rs_stmt(other, lvl, top_fns),
    }
}

fn rs_stmt(stmt: &Stmt, lvl: usize, top_fns: &std::collections::HashSet<String>) -> String {
    let pad = indent(lvl);
    match stmt {
        Stmt::Import(path) => format!("{}// import \"{}\"\n", pad, escape_str(path)),
        Stmt::ClassDecl {
            name,
            parent,
            methods,
        } => rs_stmt(&desugar_class(name, parent, methods), lvl, top_fns),
        Stmt::TryCatch {
            try_body,
            err_var,
            catch_body,
        } => {
            let mut s = format!(
                "{}let __prev_hook = std::panic::take_hook();\n{}std::panic::set_hook(Box::new(|_| {{}}));\n{}let __res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Option<SVal> {{\n",
                pad, pad, pad
            );
            for st in try_body {
                s.push_str(&rs_try_stmt(st, lvl + 1, top_fns));
            }
            s.push_str(&format!("{}    None\n{}}}));\n{}std::panic::set_hook(__prev_hook);\n", pad, pad, pad));
            if rs_stmts_have_return(try_body) {
                s.push_str(&format!("{}if let Ok(Some(__ret_val)) = __res {{ return __ret_val; }}\n", pad));
            }
            s.push_str(&format!(
                "{}if let Err(__err) = __res {{\n{}    let __msg = __err.downcast_ref::<String>().cloned().or_else(|| __err.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| \"Caught error\".to_string());\n{}    let mut {} = SVal::Str(__msg);\n",
                pad, pad, pad, err_var
            ));
            for st in catch_body {
                s.push_str(&rs_stmt(st, lvl + 1, top_fns));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::Throw(e) => format!(
            "{}panic!(\"{{}}\", {});\n",
            pad,
            rs_expr(e, lvl, top_fns)
        ),
        Stmt::Test { name, body } => {
            let mut s = format!("{}// test \"{}\"\n{}{{\n", pad, escape_str(name), pad);
            for st in body {
                s.push_str(&rs_stmt(st, lvl + 1, top_fns));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::Defer(inner) => rs_stmt(inner, lvl, top_fns),
        Stmt::Let { name, value, .. } => {
            if rs_is_global(name) && lvl == 1 {
                format!("{}__STEEL_G_{}.with(|__g| *__g.borrow_mut() = {});\n", pad, name, rs_expr(value, lvl, top_fns))
            } else {
                format!("{}let mut {} = {};\n", pad, rs_safe_ident(name), rs_expr(value, lvl, top_fns))
            }
        }
        Stmt::Assign { target, value } => match target {
            AssignTarget::Ident(n) => {
                if rs_is_global(n) {
                    format!("{}__STEEL_G_{}.with(|__g| *__g.borrow_mut() = {});\n", pad, n, rs_expr(value, lvl, top_fns))
                } else {
                    format!("{}{} = {};\n", pad, rs_safe_ident(n), rs_expr(value, lvl, top_fns))
                }
            }
            AssignTarget::Index { target, index } => format!(
                "{}{}.set({}, {});\n",
                pad,
                rs_expr(target, lvl, top_fns),
                rs_expr(index, lvl, top_fns),
                rs_expr(value, lvl, top_fns)
            ),
            AssignTarget::Member { target, field } => format!(
                "{}{}.set(SVal::Str(\"{}\".to_string()), {});\n",
                pad,
                rs_expr(target, lvl, top_fns),
                escape_str(field),
                rs_expr(value, lvl, top_fns)
            ),
        },
        Stmt::FnDecl { name, params, body } => {
            let mut s = format!(
                "{}let mut {} = SVal::Func(Rc::new(move |__args: Vec<SVal>| -> SVal {{\n",
                pad, name
            );
            for (i, (p, def)) in params.iter().enumerate() {
                let fallback = def
                    .as_ref()
                    .map(|d| rs_expr(d, lvl + 1, top_fns))
                    .unwrap_or_else(|| "SVal::Null".to_string());
                s.push_str(&format!(
                    "{}    let mut {} = __args.get({}).cloned().unwrap_or_else(|| {});\n",
                    pad, p, i, fallback
                ));
            }
            for st in body {
                s.push_str(&rs_stmt(st, lvl + 1, top_fns));
            }
            s.push_str(&format!("{}    SVal::Null\n{}}}));\n", pad, pad));
            s
        }
        Stmt::Return(expr) => match expr {
            Some(e) => format!("{}return {};\n", pad, rs_expr(e, lvl, top_fns)),
            None => format!("{}return SVal::Null;\n", pad),
        },
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let mut s = format!("{}if ({}).is_truthy() {{\n", pad, rs_expr(cond, lvl, top_fns));
            for st in then_branch {
                s.push_str(&rs_stmt(st, lvl + 1, top_fns));
            }
            if let Some(eb) = else_branch {
                s.push_str(&format!("{}}} else {{\n", pad));
                for st in eb {
                    s.push_str(&rs_stmt(st, lvl + 1, top_fns));
                }
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::While { cond, body } => {
            let mut s = format!(
                "{}while ({}).is_truthy() {{\n",
                pad,
                rs_expr(cond, lvl, top_fns)
            );
            for st in body {
                s.push_str(&rs_stmt(st, lvl + 1, top_fns));
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
                "{}for mut {} in ({}).iter_vec() {{\n",
                pad,
                var,
                rs_expr(iterable, lvl, top_fns)
            );
            for st in body {
                s.push_str(&rs_stmt(st, lvl + 1, top_fns));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::Break => format!("{}break;\n", pad),
        Stmt::Continue => format!("{}continue;\n", pad),
        Stmt::Expr(e) => format!("{}{};\n", pad, rs_expr(e, lvl, top_fns)),
    }
}

fn rs_expr(expr: &Expr, lvl: usize, top_fns: &std::collections::HashSet<String>) -> String {
    match expr {
        Expr::Number(n) => format!("SVal::Num({:.6})", n),
        Expr::String(s) => format!("SVal::Str(\"{}\".to_string())", escape_str(s)),
        Expr::Bool(b) => format!("SVal::Bool({})", b),
        Expr::Null => "SVal::Null".to_string(),
        Expr::Ident(name) => match name.as_str() {
            "println" => "print.clone()".to_string(),
            "str" => "str_fn.clone()".to_string(),
            "num" => "num_fn.clone()".to_string(),
            "int" => "int_fn.clone()".to_string(),
            "bool" => "bool_fn.clone()".to_string(),
            "type" => "type_fn.clone()".to_string(),
            other => {
                if top_fns.contains(other) {
                    format!("SVal::Func(Rc::new(__steel_fn_{}))", other)
                } else if rs_is_global(other) {
                    format!("__STEEL_G_{}.with(|__g| __g.borrow().clone())", other)
                } else {
                    format!("{}.clone()", rs_safe_ident(other))
                }
            }
        },
        Expr::Array(items) => {
            let inner: Vec<String> = items.iter().map(|e| rs_expr(e, lvl, top_fns)).collect();
            format!(
                "SVal::Arr(Rc::new(RefCell::new(vec![{}])))",
                inner.join(", ")
            )
        }
        Expr::Map(entries) => {
            let mut s = String::from("{ let mut __m = BTreeMap::new(); ");
            for (k, v) in entries {
                s.push_str(&format!(
                    "__m.insert(\"{}\".to_string(), {}); ",
                    escape_str(k),
                    rs_expr(v, lvl, top_fns)
                ));
            }
            s.push_str("SVal::Map(Rc::new(RefCell::new(__m))) }");
            s
        }
        Expr::New { class_name, args } => {
            let arg_strs: Vec<String> = args.iter().map(|a| rs_expr(a, lvl, top_fns)).collect();
            if top_fns.contains(class_name) {
                format!("{}({})", class_name, arg_strs.join(", "))
            } else {
                format!("{}.call(vec![{}])", class_name, arg_strs.join(", "))
            }
        }
        Expr::Ternary {
            cond,
            then_expr,
            else_expr,
        } => format!(
            "(if ({}).is_truthy() {{ {} }} else {{ {} }})",
            rs_expr(cond, lvl, top_fns),
            rs_expr(then_expr, lvl, top_fns),
            rs_expr(else_expr, lvl, top_fns)
        ),
        Expr::Binary { left, op, right } => {
            let l = rs_expr(left, lvl, top_fns);
            let r = rs_expr(right, lvl, top_fns);
            match op {
                BinaryOp::Add => format!("({} + {})", l, r),
                BinaryOp::Sub => format!("({} - {})", l, r),
                BinaryOp::Mul => format!("({} * {})", l, r),
                BinaryOp::Pow => format!("SVal::Num(({}).as_num().powf(({}).as_num()))", l, r),
                BinaryOp::Div => format!("({} / {})", l, r),
                BinaryOp::Mod => format!("({} % {})", l, r),
                BinaryOp::BitAnd => {
                    format!("SVal::Num(((({}).as_num() as i64) & (({}).as_num() as i64)) as f64)", l, r)
                }
                BinaryOp::BitOr => {
                    format!("SVal::Num(((({}).as_num() as i64) | (({}).as_num() as i64)) as f64)", l, r)
                }
                BinaryOp::BitXor => {
                    format!("SVal::Num(((({}).as_num() as i64) ^ (({}).as_num() as i64)) as f64)", l, r)
                }
                BinaryOp::Shl => {
                    format!("SVal::Num(((({}).as_num() as i64) << (({}).as_num() as i64)) as f64)", l, r)
                }
                BinaryOp::Shr => {
                    format!("SVal::Num(((({}).as_num() as i64) >> (({}).as_num() as i64)) as f64)", l, r)
                }
                BinaryOp::NullCoalesce => {
                    format!("{{ let __v = {}; if __v == SVal::Null {{ {} }} else {{ __v }} }}", l, r)
                }
                BinaryOp::Eq => format!("SVal::Bool({} == {})", l, r),
                BinaryOp::Neq => format!("SVal::Bool({} != {})", l, r),
                BinaryOp::Lt => format!("SVal::Bool(({}).as_num() < ({}).as_num())", l, r),
                BinaryOp::Gt => format!("SVal::Bool(({}).as_num() > ({}).as_num())", l, r),
                BinaryOp::Lte => format!("SVal::Bool(({}).as_num() <= ({}).as_num())", l, r),
                BinaryOp::Gte => format!("SVal::Bool(({}).as_num() >= ({}).as_num())", l, r),
                BinaryOp::And => {
                    format!("SVal::Bool(({}).is_truthy() && ({}).is_truthy())", l, r)
                }
                BinaryOp::Or => format!("SVal::Bool(({}).is_truthy() || ({}).is_truthy())", l, r),
            }
        }
        Expr::Unary { op, expr } => {
            let e = rs_expr(expr, lvl, top_fns);
            match op {
                UnaryOp::Neg => format!("(-{})", e),
                UnaryOp::Not => format!("(!{})", e),
                UnaryOp::BitNot => format!("SVal::Num((!(({}).as_num() as i64)) as f64)", e),
            }
        }
        Expr::Call { callee, args } => {
            let arg_strs: Vec<String> = args.iter().map(|a| rs_expr(a, lvl, top_fns)).collect();
            if let Expr::Ident(name) = &**callee {
                if top_fns.contains(name) {
                    return format!("{}({})", name, arg_strs.join(", "));
                }
            }
            format!(
                "({}).call(vec![{}])",
                rs_expr(callee, lvl, top_fns),
                arg_strs.join(", ")
            )
        }
        Expr::Index { target, index } => {
            format!(
                "({}).get({})",
                rs_expr(target, lvl, top_fns),
                rs_expr(index, lvl, top_fns)
            )
        }
        Expr::Member { target, field } => format!(
            "({}).get(SVal::Str(\"{}\".to_string()))",
            rs_expr(target, lvl, top_fns),
            escape_str(field)
        ),
        Expr::FnExpr { params, body } => {
            let pad = indent(lvl);
            let mut body_str = String::new();
            let mut bound: std::collections::HashSet<String> =
                params.iter().map(|(p, _)| rs_safe_ident(p)).collect();
            for (i, (p, def)) in params.iter().enumerate() {
                let fallback = def
                    .as_ref()
                    .map(|d| rs_expr(d, lvl + 1, top_fns))
                    .unwrap_or_else(|| "SVal::Null".to_string());
                body_str.push_str(&format!(
                    "{}    let mut {} = __args.get({}).cloned().unwrap_or_else(|| {});\n",
                    pad, rs_safe_ident(p), i, fallback
                ));
            }
            for st in body {
                body_str.push_str(&rs_stmt(st, lvl + 1, top_fns));
            }
            let mut local_decls = std::collections::HashSet::new();
            rs_collect_local_decls(body, &mut local_decls);
            for d in local_decls {
                bound.insert(rs_safe_ident(&d));
            }
            let mut raw_refs = std::collections::HashSet::new();
            rs_collect_ident_refs_stmts(body, &mut raw_refs);
            let mut captures = std::collections::BTreeSet::new();
            for r in raw_refs {
                if top_fns.contains(&r) || rs_is_global(&r) {
                    continue;
                }
                let mapped = match r.as_str() {
                    "println" => "print".to_string(),
                    "str" => "str_fn".to_string(),
                    "num" => "num_fn".to_string(),
                    "int" => "int_fn".to_string(),
                    "bool" => "bool_fn".to_string(),
                    "type" => "type_fn".to_string(),
                    other => rs_safe_ident(other),
                };
                if !bound.contains(&mapped) && body_str.contains(&format!("{}.clone()", mapped)) {
                    captures.insert(mapped);
                }
            }
            if !captures.is_empty() {
                let clones: Vec<String> = captures
                    .iter()
                    .map(|c| format!("let {} = {}.clone();", c, c))
                    .collect();
                format!(
                    "{{ {} SVal::Func(Rc::new(move |__args: Vec<SVal>| -> SVal {{\n{}{}    SVal::Null\n{}}})) }}",
                    clones.join(" "),
                    body_str,
                    pad,
                    pad
                )
            } else {
                format!(
                    "SVal::Func(Rc::new(move |__args: Vec<SVal>| -> SVal {{\n{}{}    SVal::Null\n{}}}))",
                    body_str, pad, pad
                )
            }
        }
        Expr::Range { start, end } => {
            format!(
                "__steel_range({}, {})",
                rs_expr(start, lvl, top_fns),
                rs_expr(end, lvl, top_fns)
            )
        }
        Expr::Spread(inner) => rs_expr(inner, lvl, top_fns),
    }
}

// 5. C TARGET (On-Demand Minimal Runtime + Unboxed Fast-Path)
fn c_extra_stdlib(code: &str) -> String {
    let mut s = String::new();
    if code.contains("__steel_jmp") {
        s.push_str("#include <setjmp.h>\n#if defined(__GNUC__) && !defined(__clang__)\n#pragma GCC optimize(\"O0\")\n#endif\nstatic jmp_buf __steel_jmp[32]; static int __steel_try_sp = 0; static SVal __steel_err;\n");
    }
    if code.contains("sv_upper(") || code.contains("sv_lower(") || code.contains("sv_trim(") {
        s.push_str("#include <ctype.h>\n");
    }
    if code.contains("sv_read_file(") {
        s.push_str(
            "static inline SVal sv_read_file(SVal p) {\n\
             \x20   FILE* f = fopen(p.type == ST_STR ? p.str : \"\", \"rb\"); if (!f) return sv_str(\"\");\n\
             \x20   fseek(f, 0, SEEK_END); long sz = ftell(f); rewind(f);\n\
             \x20   char* buf = (char*)calloc(1, (size_t)sz + 1); if (sz > 0) fread(buf, 1, (size_t)sz, f);\n\
             \x20   fclose(f); return sv_str(buf);\n\
             }\n",
        );
    }
    if code.contains("sv_write_file(") {
        s.push_str(
            "static inline SVal sv_write_file(SVal p, SVal c) {\n\
             \x20   FILE* f = fopen(p.type == ST_STR ? p.str : \"\", \"wb\"); if (!f) return sv_bool(false);\n\
             \x20   const char* txt = c.type == ST_STR ? c.str : \"\";\n\
             \x20   fwrite(txt, 1, strlen(txt), f); fclose(f); return sv_bool(true);\n\
             }\n",
        );
    }
    if code.contains("sv_file_exists(") {
        s.push_str(
            "static inline SVal sv_file_exists(SVal p) {\n\
             \x20   FILE* f = fopen(p.type == ST_STR ? p.str : \"\", \"rb\"); if (f) { fclose(f); return sv_bool(true); } return sv_bool(false);\n\
             }\n",
        );
    }
    if code.contains("sv_env(") {
        s.push_str(
            "static inline SVal sv_env(SVal k) {\n\
             \x20   const char* v = getenv(k.type == ST_STR ? k.str : \"\"); return v ? sv_str(v) : sv_null();\n\
             }\n",
        );
    }
    if code.contains("sv_num_cast(") {
        s.push_str(
            "static inline SVal sv_num_cast(SVal v) {\n\
             \x20   if (v.type == ST_NUM || v.type == ST_BOOL) return sv_num(v.num);\n\
             \x20   if (v.type == ST_STR && v.str) return sv_num(strtod(v.str, NULL));\n\
             \x20   return sv_num(0.0);\n\
             }\n",
        );
    }
    if code.contains("sv_upper(") {
        s.push_str(
            "static inline SVal sv_upper(SVal v) {\n\
             \x20   const char* src = v.type == ST_STR ? v.str : \"\"; size_t n = strlen(src);\n\
             \x20   char* b = (char*)calloc(1, n + 1); for (size_t i = 0; i < n; i++) b[i] = (char)toupper((unsigned char)src[i]); return sv_str(b);\n\
             }\n",
        );
    }
    if code.contains("sv_lower(") {
        s.push_str(
            "static inline SVal sv_lower(SVal v) {\n\
             \x20   const char* src = v.type == ST_STR ? v.str : \"\"; size_t n = strlen(src);\n\
             \x20   char* b = (char*)calloc(1, n + 1); for (size_t i = 0; i < n; i++) b[i] = (char)tolower((unsigned char)src[i]); return sv_str(b);\n\
             }\n",
        );
    }
    if code.contains("sv_trim(") {
        s.push_str(
            "static inline SVal sv_trim(SVal v) {\n\
             \x20   const char* src = v.type == ST_STR ? v.str : \"\"; while (*src && isspace((unsigned char)*src)) src++;\n\
             \x20   size_t n = strlen(src); while (n > 0 && isspace((unsigned char)src[n - 1])) n--;\n\
             \x20   char* b = (char*)calloc(1, n + 1); memcpy(b, src, n); return sv_str(b);\n\
             }\n",
        );
    }
    if code.contains("sv_contains(") {
        s.push_str(
            "static inline SVal sv_contains(SVal h, SVal n) {\n\
             \x20   if (h.type == ST_STR && n.type == ST_STR) return sv_bool(strstr(h.str, n.str) != NULL);\n\
             \x20   if (h.type == ST_MAP && n.type == ST_STR) { for (int i = 0; i < h.map->len; i++) if (strcmp(h.map->keys[i], n.str) == 0) return sv_bool(true); return sv_bool(false); }\n\
             \x20   if (h.type == ST_ARR) { for (int i = 0; i < h.arr->len; i++) if (sv_eq(h.arr->data[i], n)) return sv_bool(true); return sv_bool(false); }\n\
             \x20   return sv_bool(false);\n\
             }\n",
        );
    }
    if code.contains("sv_starts_with(") {
        s.push_str(
            "static inline SVal sv_starts_with(SVal h, SVal p) {\n\
             \x20   if (h.type != ST_STR || p.type != ST_STR) return sv_bool(false);\n\
             \x20   size_t pl = strlen(p.str);\n\
             \x20   return sv_bool(strncmp(h.str, p.str, pl) == 0);\n\
             }\n",
        );
    }
    if code.contains("sv_ends_with(") {
        s.push_str(
            "static inline SVal sv_ends_with(SVal h, SVal p) {\n\
             \x20   if (h.type != ST_STR || p.type != ST_STR) return sv_bool(false);\n\
             \x20   size_t hl = strlen(h.str), pl = strlen(p.str);\n\
             \x20   if (pl > hl) return sv_bool(false);\n\
             \x20   return sv_bool(strcmp(h.str + (hl - pl), p.str) == 0);\n\
             }\n",
        );
    }
    if code.contains("sv_split(") {
        s.push_str(
            "static inline SVal sv_split(SVal s, SVal sep) {\n\
             \x20   SVal arr = sv_arr_new(0, NULL);\n\
             \x20   const char* src = s.type == ST_STR ? s.str : \"\";\n\
             \x20   const char* sp = sep.type == ST_STR ? sep.str : \"\";\n\
             \x20   size_t sl = strlen(sp);\n\
             \x20   if (sl == 0) { for (size_t i = 0; src[i]; i++) { char* c = (char*)calloc(1, 2); c[0] = src[i]; sv_push(arr, sv_str(c)); } return arr; }\n\
             \x20   const char* cur = src;\n\
             \x20   const char* hit;\n\
             \x20   while ((hit = strstr(cur, sp)) != NULL) {\n\
             \x20       size_t len = (size_t)(hit - cur);\n\
             \x20       char* part = (char*)calloc(1, len + 1); memcpy(part, cur, len);\n\
             \x20       sv_push(arr, sv_str(part)); cur = hit + sl;\n\
             \x20   }\n\
             \x20   size_t rem = strlen(cur); char* last = (char*)calloc(1, rem + 1); memcpy(last, cur, rem);\n\
             \x20   sv_push(arr, sv_str(last)); return arr;\n\
             }\n",
        );
    }
    if code.contains("sv_join(") {
        s.push_str(
            "static inline SVal sv_join(SVal arr, SVal sep) {\n\
             \x20   if (arr.type != ST_ARR) return sv_str(\"\");\n\
             \x20   const char* sp = sep.type == ST_STR ? sep.str : \"\";\n\
             \x20   size_t sl = strlen(sp), cap = 64, len = 0;\n\
             \x20   char* buf = (char*)calloc(1, cap);\n\
             \x20   for (int i = 0; i < arr.arr->len; i++) {\n\
             \x20       char tmp[64]; const char* part = \"\";\n\
             \x20       SVal v = arr.arr->data[i];\n\
             \x20       if (v.type == ST_STR) part = v.str;\n\
             \x20       else if (v.type == ST_NUM) { snprintf(tmp, sizeof(tmp), \"%g\", v.num); part = tmp; }\n\
             \x20       else if (v.type == ST_BOOL) part = v.num != 0.0 ? \"true\" : \"false\";\n\
             \x20       else if (v.type == ST_NULL) part = \"null\";\n\
             \x20       size_t pl = strlen(part), need = len + (i > 0 ? sl : 0) + pl + 1;\n\
             \x20       while (need >= cap) { cap *= 2; buf = (char*)realloc(buf, cap); }\n\
             \x20       if (i > 0) { memcpy(buf + len, sp, sl); len += sl; }\n\
             \x20       memcpy(buf + len, part, pl); len += pl; buf[len] = '\\0';\n\
             \x20   }\n\
             \x20   return sv_str(buf);\n\
             }\n",
        );
    }
    if code.contains("sv_replace(") {
        s.push_str(
            "static inline SVal sv_replace(SVal s, SVal a, SVal b) {\n\
             \x20   const char* src = s.type == ST_STR ? s.str : \"\";\n\
             \x20   const char* old_s = a.type == ST_STR ? a.str : \"\";\n\
             \x20   const char* new_s = b.type == ST_STR ? b.str : \"\";\n\
             \x20   size_t ol = strlen(old_s), nl = strlen(new_s);\n\
             \x20   if (ol == 0) return s;\n\
             \x20   size_t cap = strlen(src) + 64, len = 0; char* buf = (char*)calloc(1, cap);\n\
             \x20   const char* cur = src; const char* hit;\n\
             \x20   while ((hit = strstr(cur, old_s)) != NULL) {\n\
             \x20       size_t seg = (size_t)(hit - cur);\n\
             \x20       while (len + seg + nl + 1 >= cap) { cap *= 2; buf = (char*)realloc(buf, cap); }\n\
             \x20       memcpy(buf + len, cur, seg); len += seg;\n\
             \x20       memcpy(buf + len, new_s, nl); len += nl; cur = hit + ol;\n\
             \x20   }\n\
             \x20   size_t rem = strlen(cur);\n\
             \x20   while (len + rem + 1 >= cap) { cap *= 2; buf = (char*)realloc(buf, cap); }\n\
             \x20   memcpy(buf + len, cur, rem); buf[len + rem] = '\\0'; return sv_str(buf);\n\
             }\n",
        );
    }
    if code.contains("sv_reverse(") {
        s.push_str(
            "static inline SVal sv_reverse(SVal v) {\n\
             \x20   if (v.type == ST_ARR) { SVal out = sv_arr_new(0, NULL); for (int i = v.arr->len - 1; i >= 0; i--) sv_push(out, v.arr->data[i]); return out; }\n\
             \x20   if (v.type == ST_STR) { size_t n = strlen(v.str); char* b = (char*)calloc(1, n + 1); for (size_t i = 0; i < n; i++) b[i] = v.str[n - 1 - i]; return sv_str(b); }\n\
             \x20   return sv_null();\n\
             }\n",
        );
    }
    if code.contains("sv_keys(") {
        s.push_str(
            "static inline SVal sv_keys(SVal v) {\n\
             \x20   SVal out = sv_arr_new(0, NULL);\n\
             \x20   if (v.type == ST_MAP) { for (int i = 0; i < v.map->len; i++) sv_push(out, sv_str(v.map->keys[i])); }\n\
             \x20   return out;\n\
             }\n",
        );
    }
    if code.contains("sv_values(") {
        s.push_str(
            "static inline SVal sv_values(SVal v) {\n\
             \x20   SVal out = sv_arr_new(0, NULL);\n\
             \x20   if (v.type == ST_MAP) { for (int i = 0; i < v.map->len; i++) sv_push(out, v.map->vals[i]); }\n\
             \x20   return out;\n\
             }\n",
        );
    }
    if code.contains("sv_slice(") {
        s.push_str(
            "static inline SVal sv_slice(SVal v, SVal s, SVal e) {\n\
             \x20   int si = (int)s.num;\n\
             \x20   if (v.type == ST_ARR) {\n\
             \x20       int ei = (e.type == ST_NUM) ? (int)e.num : v.arr->len;\n\
             \x20       if (si < 0) si = 0; if (ei > v.arr->len) ei = v.arr->len; if (si > ei) si = ei;\n\
             \x20       SVal out = sv_arr_new(0, NULL);\n\
             \x20       for (int i = si; i < ei; i++) sv_push(out, v.arr->data[i]);\n\
             \x20       return out;\n\
             \x20   }\n\
             \x20   if (v.type == ST_STR) {\n\
             \x20       int n = (int)strlen(v.str); int ei = (e.type == ST_NUM) ? (int)e.num : n;\n\
             \x20       if (si < 0) si = 0; if (ei > n) ei = n; if (si > ei) si = ei;\n\
             \x20       char* b = (char*)calloc(1, (size_t)(ei - si) + 1); memcpy(b, v.str + si, (size_t)(ei - si));\n\
             \x20       return sv_str(b);\n\
             \x20   }\n\
             \x20   return sv_null();\n\
             }\n",
        );
    }
    if code.contains("sv_sort(") {
        s.push_str(
            "static inline const char* __sv_sort_key(SVal v, char* buf, size_t n) {\n\
             \x20   if (v.type == ST_STR && v.str) return v.str;\n\
             \x20   if (v.type == ST_NUM) { snprintf(buf, n, \"%g\", v.num); return buf; }\n\
             \x20   if (v.type == ST_BOOL) return v.num != 0.0 ? \"true\" : \"false\";\n\
             \x20   return \"null\";\n\
             }\n\
             static inline SVal sv_sort(SVal v) {\n\
             \x20   SVal out = sv_arr_new(0, NULL);\n\
             \x20   if (v.type != ST_ARR) return out;\n\
             \x20   for (int i = 0; i < v.arr->len; i++) sv_push(out, v.arr->data[i]);\n\
             \x20   for (int i = 0; i < out.arr->len; i++) {\n\
             \x20       for (int j = i + 1; j < out.arr->len; j++) {\n\
             \x20           char bi[64], bj[64];\n\
             \x20           if (strcmp(__sv_sort_key(out.arr->data[j], bj, sizeof(bj)), __sv_sort_key(out.arr->data[i], bi, sizeof(bi))) < 0) {\n\
             \x20               SVal tmp = out.arr->data[i]; out.arr->data[i] = out.arr->data[j]; out.arr->data[j] = tmp;\n\
             \x20           }\n\
             \x20       }\n\
             \x20   }\n\
             \x20   return out;\n\
             }\n",
        );
    }
    if code.contains("sv_range_step(") {
        s.push_str(
            "static inline SVal sv_range_step(SVal a, SVal b, SVal st) {\n\
             \x20   int s = (int)a.num, e = (int)b.num, step = (int)st.num;\n\
             \x20   SVal r = sv_arr_new(0, NULL);\n\
             \x20   if (step > 0) { for (int i = s; i < e; i += step) sv_push(r, sv_num(i)); }\n\
             \x20   else if (step < 0) { for (int i = s; i > e; i += step) sv_push(r, sv_num(i)); }\n\
             \x20   return r;\n\
             }\n",
        );
    }
    if code.contains("sv_type(") {
        s.push_str(
            "static inline SVal sv_type(SVal v) {\n\
             \x20   switch (v.type) {\n\
             \x20       case ST_NULL: return sv_str(\"null\");\n\
             \x20       case ST_BOOL: return sv_str(\"boolean\");\n\
             \x20       case ST_NUM: return sv_str(\"number\");\n\
             \x20       case ST_STR: return sv_str(\"string\");\n\
             \x20       case ST_ARR: return sv_str(\"array\");\n\
             \x20       case ST_MAP: return sv_str(\"map\");\n\
             \x20       default: return sv_str(\"function\");\n\
             \x20   }\n\
             }\n",
        );
    }
    if code.contains("sv_int_cast(") {
        s.push_str(
            "static inline SVal sv_int_cast(SVal v) {\n\
             \x20   double d = (v.type == ST_STR && v.str) ? strtod(v.str, NULL) : ((v.type == ST_NUM || v.type == ST_BOOL) ? v.num : 0.0);\n\
             \x20   return sv_num(trunc(d));\n\
             }\n",
        );
    }
    if code.contains("sv_assert(") {
        s.push_str(
            "static inline SVal sv_assert(SVal c, SVal m) {\n\
             \x20   if (!sv_truthy(c)) { fprintf(stderr, \"Assertion failed: %s\\n\", (m.type == ST_STR && m.str) ? m.str : \"\"); exit(1); }\n\
             \x20   return sv_bool(true);\n\
             }\n",
        );
    }
    if code.contains("sv_has_key(") {
        s.push_str(
            "static inline SVal sv_has_key(SVal m, SVal k) {\n\
             \x20   if (m.type == ST_MAP && k.type == ST_STR) { for (int i = 0; i < m.map->len; i++) if (strcmp(m.map->keys[i], k.str) == 0) return sv_bool(true); }\n\
             \x20   return sv_bool(false);\n\
             }\n",
        );
    }
    if code.contains("sv_delete_key(") {
        s.push_str(
            "static inline SVal sv_delete_key(SVal m, SVal k) {\n\
             \x20   if (m.type == ST_MAP && k.type == ST_STR) {\n\
             \x20       for (int i = 0; i < m.map->len; i++) {\n\
             \x20           if (strcmp(m.map->keys[i], k.str) == 0) {\n\
             \x20               for (int j = i; j + 1 < m.map->len; j++) { m.map->keys[j] = m.map->keys[j + 1]; m.map->vals[j] = m.map->vals[j + 1]; }\n\
             \x20               m.map->len--; return sv_bool(true);\n\
             \x20           }\n\
             \x20       }\n\
             \x20   }\n\
             \x20   return sv_bool(false);\n\
             }\n",
        );
    }
    if code.contains("sv_concat(") {
        s.push_str(
            "static inline SVal sv_concat(SVal a, SVal b) {\n\
             \x20   SVal out = sv_arr_new(0, NULL);\n\
             \x20   if (a.type == ST_ARR) for (int i = 0; i < a.arr->len; i++) sv_push(out, a.arr->data[i]);\n\
             \x20   if (b.type == ST_ARR) for (int i = 0; i < b.arr->len; i++) sv_push(out, b.arr->data[i]);\n\
             \x20   return out;\n\
             }\n",
        );
    }
    if code.contains("sv_index_of(") {
        s.push_str(
            "static inline SVal sv_index_of(SVal h, SVal n) {\n\
             \x20   if (h.type == ST_STR && n.type == ST_STR) { const char* p = strstr(h.str, n.str); return sv_num(p ? (double)(p - h.str) : -1.0); }\n\
             \x20   if (h.type == ST_ARR) { for (int i = 0; i < h.arr->len; i++) if (sv_eq(h.arr->data[i], n)) return sv_num(i); }\n\
             \x20   return sv_num(-1.0);\n\
             }\n",
        );
    }
    if code.contains("sv_repeat(") {
        s.push_str(
            "static inline SVal sv_repeat(SVal v, SVal n) {\n\
             \x20   const char* src = (v.type == ST_STR && v.str) ? v.str : \"\"; int c = (int)n.num; if (c < 0) c = 0;\n\
             \x20   size_t sl = strlen(src); char* b = (char*)calloc(1, sl * (size_t)c + 1);\n\
             \x20   for (int i = 0; i < c; i++) memcpy(b + (size_t)i * sl, src, sl); return sv_str(b);\n\
             }\n",
        );
    }
    if code.contains("sv_pad_start(") {
        s.push_str(
            "static inline SVal sv_pad_start(SVal v, SVal w, SVal ch) {\n\
             \x20   const char* src = (v.type == ST_STR && v.str) ? v.str : \"\"; const char* p = (ch.type == ST_STR && ch.str) ? ch.str : \" \";\n\
             \x20   size_t sl = strlen(src), pl = strlen(p), ww = (w.num > 0) ? (size_t)w.num : 0;\n\
             \x20   if (sl >= ww || pl == 0) return sv_str(src);\n\
             \x20   size_t need = ww - sl; char* b = (char*)calloc(1, ww + 1);\n\
             \x20   for (size_t i = 0; i < need; i++) b[i] = p[i % pl];\n\
             \x20   memcpy(b + need, src, sl); return sv_str(b);\n\
             }\n",
        );
    }
    if code.contains("sv_pad_end(") {
        s.push_str(
            "static inline SVal sv_pad_end(SVal v, SVal w, SVal ch) {\n\
             \x20   const char* src = (v.type == ST_STR && v.str) ? v.str : \"\"; const char* p = (ch.type == ST_STR && ch.str) ? ch.str : \" \";\n\
             \x20   size_t sl = strlen(src), pl = strlen(p), ww = (w.num > 0) ? (size_t)w.num : 0;\n\
             \x20   if (sl >= ww || pl == 0) return sv_str(src);\n\
             \x20   size_t need = ww - sl; char* b = (char*)calloc(1, ww + 1);\n\
             \x20   memcpy(b, src, sl); for (size_t i = 0; i < need; i++) b[sl + i] = p[i % pl]; return sv_str(b);\n\
             }\n",
        );
    }
    if code.contains("sv_char_code(") {
        s.push_str(
            "static inline SVal sv_char_code(SVal v) { return (v.type == ST_STR && v.str && v.str[0]) ? sv_num((unsigned char)v.str[0]) : sv_num(0); }\n",
        );
    }
    if code.contains("sv_from_char_code(") {
        s.push_str(
            "static inline SVal sv_from_char_code(SVal n) { char* b = (char*)calloc(1, 2); b[0] = (char)((int)n.num); return sv_str(b); }\n",
        );
    }
    if code.contains("sv_find_hof(") {
        s.push_str(
            "static inline SVal sv_find_hof(SVal arr, SVal fn) { if (arr.type == ST_ARR) { SVal env_null = sv_null(); for (int i = 0; i < arr.arr->len; i++) { SVal a[1] = { arr.arr->data[i] }; if (sv_truthy(sv_call_fn(fn, 1, a))) return arr.arr->data[i]; } (void)env_null; } return sv_null(); }\n",
        );
    }
    if code.contains("sv_any_hof(") {
        s.push_str(
            "static inline SVal sv_any_hof(SVal arr, SVal fn) { if (arr.type == ST_ARR) { for (int i = 0; i < arr.arr->len; i++) { SVal a[1] = { arr.arr->data[i] }; if (sv_truthy(sv_call_fn(fn, 1, a))) return sv_bool(true); } } return sv_bool(false); }\n",
        );
    }
    if code.contains("sv_all_hof(") {
        s.push_str(
            "static inline SVal sv_all_hof(SVal arr, SVal fn) { if (arr.type == ST_ARR) { for (int i = 0; i < arr.arr->len; i++) { SVal a[1] = { arr.arr->data[i] }; if (!sv_truthy(sv_call_fn(fn, 1, a))) return sv_bool(false); } return sv_bool(true); } return sv_bool(false); }\n",
        );
    }
    if code.contains("sv_sum(") {
        s.push_str(
            "static inline SVal sv_sum(SVal arr) { double t = 0; if (arr.type == ST_ARR) for (int i = 0; i < arr.arr->len; i++) if (arr.arr->data[i].type == ST_NUM) t += arr.arr->data[i].num; return sv_num(t); }\n",
        );
    }
    if code.contains("sv_avg(") {
        s.push_str(
            "static inline SVal sv_avg(SVal arr) { if (arr.type != ST_ARR || arr.arr->len == 0) return sv_num(0); double t = 0; for (int i = 0; i < arr.arr->len; i++) if (arr.arr->data[i].type == ST_NUM) t += arr.arr->data[i].num; return sv_num(t / arr.arr->len); }\n",
        );
    }
    if code.contains("sv_unique(") {
        s.push_str(
            "static inline SVal sv_unique(SVal arr) { SVal out = sv_arr_new(0, NULL); if (arr.type == ST_ARR) { for (int i = 0; i < arr.arr->len; i++) { bool dup = false; for (int j = 0; j < out.arr->len; j++) if (sv_eq(out.arr->data[j], arr.arr->data[i])) { dup = true; break; } if (!dup) sv_push(out, arr.arr->data[i]); } } return out; }\n",
        );
    }
    if code.contains("sv_flat(") {
        s.push_str(
            "static inline SVal sv_flat(SVal arr) { SVal out = sv_arr_new(0, NULL); if (arr.type == ST_ARR) { for (int i = 0; i < arr.arr->len; i++) { SVal it = arr.arr->data[i]; if (it.type == ST_ARR) { for (int j = 0; j < it.arr->len; j++) sv_push(out, it.arr->data[j]); } else sv_push(out, it); } } return out; }\n",
        );
    }
    if code.contains("sv_zip(") {
        s.push_str(
            "static inline SVal sv_zip(SVal a, SVal b) { SVal out = sv_arr_new(0, NULL); if (a.type == ST_ARR && b.type == ST_ARR) { int n = a.arr->len < b.arr->len ? a.arr->len : b.arr->len; for (int i = 0; i < n; i++) { SVal pair = sv_arr_new(0, NULL); sv_push(pair, a.arr->data[i]); sv_push(pair, b.arr->data[i]); sv_push(out, pair); } } return out; }\n",
        );
    }
    if code.contains("sv_enumerate(") {
        s.push_str(
            "static inline SVal sv_enumerate(SVal a) { SVal out = sv_arr_new(0, NULL); if (a.type == ST_ARR) { for (int i = 0; i < a.arr->len; i++) { SVal pair = sv_arr_new(0, NULL); sv_push(pair, sv_num(i)); sv_push(pair, a.arr->data[i]); sv_push(out, pair); } } return out; }\n",
        );
    }
    if code.contains("sv_append_file(") {
        s.push_str(
            "static inline SVal sv_append_file(SVal p, SVal c) { FILE* f = fopen(p.type == ST_STR ? p.str : \"\", \"ab\"); if (!f) return sv_bool(false); const char* txt = c.type == ST_STR ? c.str : \"\"; fwrite(txt, 1, strlen(txt), f); fclose(f); return sv_bool(true); }\n",
        );
    }
    if code.contains("sv_remove_file(") {
        s.push_str(
            "static inline SVal sv_remove_file(SVal p) { return sv_bool(remove(p.type == ST_STR ? p.str : \"\") == 0); }\n",
        );
    }
    if code.contains("sv_clock(") {
        s.push_str("#include <time.h>\nstatic inline SVal sv_clock(void) { return sv_num((double)time(NULL)); }\n");
    }
    if code.contains("sv_hash(") {
        s.push_str(
            "#include <stdint.h>\nstatic inline SVal sv_hash(SVal v) {\n\
             \x20   const char* src = (v.type == ST_STR && v.str) ? v.str : \"\";\n\
             \x20   uint64_t h = 0xcbf29ce484222325ULL;\n\
             \x20   for (const unsigned char* p = (const unsigned char*)src; *p; p++) { h ^= (uint64_t)(*p); h *= 0x0100000001b3ULL; }\n\
             \x20   char* out = (char*)calloc(1, 17); snprintf(out, 17, \"%016llx\", (unsigned long long)h); return sv_str(out);\n\
             }\n",
        );
    }
    if code.contains("sv_json_stringify(") {
        s.push_str(
            "static void __sv_js_app(char** buf, size_t* len, size_t* cap, const char* s) {\n\
             \x20   size_t sl = strlen(s); while (*len + sl + 1 >= *cap) { *cap *= 2; *buf = (char*)realloc(*buf, *cap); }\n\
             \x20   memcpy(*buf + *len, s, sl); *len += sl; (*buf)[*len] = '\\0';\n\
             }\n\
             static void __sv_js_esc(char** buf, size_t* len, size_t* cap, const char* s) {\n\
             \x20   __sv_js_app(buf, len, cap, \"\\\"\");\n\
             \x20   for (; *s; s++) { char c = *s; if (c == '\\\\') __sv_js_app(buf, len, cap, \"\\\\\\\\\"); else if (c == '\"') __sv_js_app(buf, len, cap, \"\\\\\\\"\"); else if (c == '\\n') __sv_js_app(buf, len, cap, \"\\\\n\"); else if (c == '\\r') __sv_js_app(buf, len, cap, \"\\\\r\"); else if (c == '\\t') __sv_js_app(buf, len, cap, \"\\\\t\"); else { char t[2] = {c, 0}; __sv_js_app(buf, len, cap, t); } }\n\
             \x20   __sv_js_app(buf, len, cap, \"\\\"\");\n\
             }\n\
             static void __sv_js_val(char** buf, size_t* len, size_t* cap, SVal v) {\n\
             \x20   if (v.type == ST_NULL) __sv_js_app(buf, len, cap, \"null\");\n\
             \x20   else if (v.type == ST_BOOL) __sv_js_app(buf, len, cap, v.num != 0.0 ? \"true\" : \"false\");\n\
             \x20   else if (v.type == ST_NUM) { char t[64]; snprintf(t, sizeof(t), \"%g\", v.num); __sv_js_app(buf, len, cap, t); }\n\
             \x20   else if (v.type == ST_STR) __sv_js_esc(buf, len, cap, v.str ? v.str : \"\");\n\
             \x20   else if (v.type == ST_ARR) { __sv_js_app(buf, len, cap, \"[\"); for (int i = 0; i < v.arr->len; i++) { if (i > 0) __sv_js_app(buf, len, cap, \",\"); __sv_js_val(buf, len, cap, v.arr->data[i]); } __sv_js_app(buf, len, cap, \"]\"); }\n\
             \x20   else if (v.type == ST_MAP) { __sv_js_app(buf, len, cap, \"{\"); for (int i = 0; i < v.map->len; i++) { if (i > 0) __sv_js_app(buf, len, cap, \",\"); __sv_js_esc(buf, len, cap, v.map->keys[i]); __sv_js_app(buf, len, cap, \":\"); __sv_js_val(buf, len, cap, v.map->vals[i]); } __sv_js_app(buf, len, cap, \"}\"); }\n\
             \x20   else __sv_js_app(buf, len, cap, \"null\");\n\
             }\n\
             static inline SVal sv_json_stringify(SVal v) { size_t cap = 64, len = 0; char* buf = (char*)calloc(1, cap); __sv_js_val(&buf, &len, &cap, v); return sv_str(buf); }\n",
        );
    }
    if code.contains("sv_json_parse(") {
        s.push_str(
            "static void __sv_jp_ws(const char* s, size_t* i) { while (s[*i] && (s[*i]==' '||s[*i]=='\\t'||s[*i]=='\\n'||s[*i]=='\\r')) (*i)++; }\n\
             static SVal __sv_jp_val(const char* s, size_t* i);\n\
             static char* __sv_jp_str(const char* s, size_t* i) {\n\
             \x20   (*i)++; size_t cap = 32, len = 0; char* out = (char*)calloc(1, cap);\n\
             \x20   while (s[*i]) { char c = s[(*i)++]; if (c == '\"') break; if (c == '\\\\' && s[*i]) { char e = s[(*i)++]; if (e=='n') c='\\n'; else if (e=='r') c='\\r'; else if (e=='t') c='\\t'; else c=e; } if (len + 2 >= cap) { cap *= 2; out = (char*)realloc(out, cap); } out[len++] = c; out[len] = '\\0'; }\n\
             \x20   return out;\n\
             }\n\
             static SVal __sv_jp_val(const char* s, size_t* i) {\n\
             \x20   __sv_jp_ws(s, i); char c = s[*i]; if (!c) return sv_null();\n\
             \x20   if (c == '\"') return sv_str(__sv_jp_str(s, i));\n\
             \x20   if (c == '[') { (*i)++; SVal arr = sv_arr_new(0, NULL); __sv_jp_ws(s, i); if (s[*i]==']') { (*i)++; return arr; } while (s[*i]) { sv_push(arr, __sv_jp_val(s, i)); __sv_jp_ws(s, i); if (s[*i]==',') { (*i)++; continue; } if (s[*i]==']') (*i)++; break; } return arr; }\n\
             \x20   if (c == '{') { (*i)++; SVal mp = sv_map_new(); __sv_jp_ws(s, i); if (s[*i]=='}') { (*i)++; return mp; } while (s[*i]) { __sv_jp_ws(s, i); char* k = __sv_jp_str(s, i); __sv_jp_ws(s, i); if (s[*i]==':') (*i)++; sv_map_set(mp, k, __sv_jp_val(s, i)); __sv_jp_ws(s, i); if (s[*i]==',') { (*i)++; continue; } if (s[*i]=='}') (*i)++; break; } return mp; }\n\
             \x20   if (strncmp(s + *i, \"true\", 4) == 0) { *i += 4; return sv_bool(true); }\n\
             \x20   if (strncmp(s + *i, \"false\", 5) == 0) { *i += 5; return sv_bool(false); }\n\
             \x20   if (strncmp(s + *i, \"null\", 4) == 0) { *i += 4; return sv_null(); }\n\
             \x20   char* end = NULL; double d = strtod(s + *i, &end); if (end && end > s + *i) { *i = (size_t)(end - s); return sv_num(d); } return sv_null();\n\
             }\n\
             static inline SVal sv_json_parse(SVal v) { const char* src = (v.type == ST_STR && v.str) ? v.str : \"null\"; size_t i = 0; return __sv_jp_val(src, &i); }\n",
        );
    }
    s
}

fn emit_c(program: &Program) -> String {
    if let Some(pure_c) = try_emit_human_c(program) {
        return pure_c;
    }

    let mut lambdas = String::new();
    let mut lambda_id = 0;
    let mut top_protos = String::new();
    let mut top_fns = String::new();
    let mut main_body = String::new();
    let mut declared = std::collections::HashSet::new();

    C_TOP_FNS.with(|f| {
        let mut set = f.borrow_mut();
        set.clear();
        for stmt in &program.statements {
            match stmt {
                Stmt::FnDecl { name, .. } | Stmt::ClassDecl { name, .. } => {
                    set.insert(name.clone());
                }
                _ => {}
            }
        }
    });
    C_CUR_LOCALS.with(|l| l.borrow_mut().clear());

    for stmt in &program.statements {
        match stmt {
            Stmt::FnDecl { name, params, body } => {
                let p_sig: Vec<String> = params.iter().map(|(p, _)| format!("SVal {}", c_safe_ident(p))).collect();
                let sig = if p_sig.is_empty() { "void".to_string() } else { p_sig.join(", ") };
                top_protos.push_str(&format!("SVal {}({});\n", c_safe_ident(name), sig));
                top_protos.push_str(&format!("SVal __st_fn_{}(SVal, int, SVal*);\n", c_safe_ident(name)));
                top_fns.push_str(&c_top_fn(name, params, body, &mut lambdas, &mut lambda_id));
            }
            Stmt::ClassDecl {
                name,
                parent,
                methods,
            } => {
                if let Stmt::FnDecl { name, params, body } = desugar_class(name, parent, methods) {
                    let p_sig: Vec<String> = params.iter().map(|(p, _)| format!("SVal {}", c_safe_ident(p))).collect();
                    let sig = if p_sig.is_empty() { "void".to_string() } else { p_sig.join(", ") };
                    top_protos.push_str(&format!("SVal {}({});\n", c_safe_ident(&name), sig));
                    top_protos.push_str(&format!("SVal __st_fn_{}(SVal, int, SVal*);\n", c_safe_ident(&name)));
                    top_fns.push_str(&c_top_fn(
                        &name,
                        &params,
                        &body,
                        &mut lambdas,
                        &mut lambda_id,
                    ));
                }
            }
            other => {
                main_body.push_str(&c_stmt(
                    other,
                    1,
                    &mut declared,
                    &mut lambdas,
                    &mut lambda_id,
                ));
            }
        }
    }

    let user_code = format!("{}{}{}", lambdas, top_fns, main_body);
    let needs_arr = user_code.contains("sv_arr_new")
        || user_code.contains("sv_push")
        || user_code.contains("sv_pop")
        || user_code.contains("sv_range")
        || user_code.contains(".arr")
        || user_code.contains("sv_get")
        || user_code.contains("sv_set")
        || user_code.contains("sv_map_hof")
        || user_code.contains("sv_filter_hof")
        || user_code.contains("sv_reduce_hof");
    let needs_map = user_code.contains("sv_map_new")
        || user_code.contains("sv_map_set")
        || user_code.contains("sv_map_get")
        || user_code.contains("sv_get")
        || user_code.contains("sv_set");
    let needs_add = user_code.contains("sv_add(");
    let needs_eq = user_code.contains("sv_eq(");
    let needs_truthy = user_code.contains("sv_truthy(");
    let needs_len = user_code.contains("sv_len(");
    let needs_hof = user_code.contains("sv_map_hof")
        || user_code.contains("sv_filter_hof")
        || user_code.contains("sv_reduce_hof");
    let needs_arena = needs_arr || needs_map || needs_add;

    let mut out = String::from(
        "#include <stdio.h>\n\
         #include <stdlib.h>\n\
         #include <string.h>\n\
         #include <stdbool.h>\n\
         #include <stdarg.h>\n\
         #include <math.h>\n\n",
    );

    if !needs_arr && !needs_map && !needs_hof {
        // Ultra-clean, minimal C struct when program only uses numbers, strings, and booleans!
        out.push_str(
            "typedef enum { ST_NULL, ST_BOOL, ST_NUM, ST_STR } SType;\n\
             typedef struct { SType type; double num; const char* str; } SVal;\n\
             static inline SVal sv_null(void) { return (SVal){ST_NULL, 0, NULL}; }\n\
             static inline SVal sv_bool(bool b) { return (SVal){ST_BOOL, b ? 1.0 : 0.0, NULL}; }\n\
             static inline SVal sv_num(double n) { return (SVal){ST_NUM, n, NULL}; }\n\
             static inline SVal sv_str(const char* s) { return (SVal){ST_STR, 0, s}; }\n",
        );
        if needs_truthy {
            out.push_str(
                "static inline bool sv_truthy(SVal v) {\n\
                 \x20   return v.type == ST_STR ? (v.str && v.str[0] != '\\0') : (v.type != ST_NULL && v.num != 0.0);\n\
                 }\n",
            );
        }
        if needs_eq {
            out.push_str(
                "static inline bool sv_eq(SVal a, SVal b) {\n\
                 \x20   if (a.type != b.type) return false;\n\
                 \x20   return a.type == ST_STR ? strcmp(a.str, b.str) == 0 : a.num == b.num;\n\
                 }\n",
            );
        }
        if needs_add || user_code.contains("sv_str_cast(") {
            out.push_str(
                "static SVal sv_str_cast(SVal v) {\n\
                 \x20   if (v.type == ST_STR) return v;\n\
                 \x20   char* b = (char*)calloc(1, 64);\n\
                 \x20   if (v.type == ST_NULL) snprintf(b, 64, \"null\");\n\
                 \x20   else if (v.type == ST_BOOL) snprintf(b, 64, \"%s\", v.num != 0.0 ? \"true\" : \"false\");\n\
                 \x20   else snprintf(b, 64, floor(v.num) == v.num ? \"%.0f\" : \"%g\", v.num);\n\
                 \x20   return sv_str(b);\n\
                 }\n",
            );
        }
        if needs_add {
            out.push_str(
                "static SVal sv_add(SVal a, SVal b) {\n\
                 \x20   if (a.type == ST_NUM && b.type == ST_NUM) return sv_num(a.num + b.num);\n\
                 \x20   SVal sa = sv_str_cast(a), sb = sv_str_cast(b);\n\
                 \x20   char* r = (char*)calloc(1, strlen(sa.str) + strlen(sb.str) + 1);\n\
                 \x20   strcpy(r, sa.str); strcat(r, sb.str);\n\
                 \x20   return sv_str(r);\n\
                 }\n",
            );
        }
        if user_code.contains("sv_print_one(") {
            out.push_str(
                "static void sv_print_one(SVal v) {\n\
                 \x20   if (v.type == ST_STR) printf(\"%s\", v.str);\n\
                 \x20   else if (v.type == ST_NUM) printf(floor(v.num) == v.num ? \"%.0f\" : \"%g\", v.num);\n\
                 \x20   else if (v.type == ST_BOOL) printf(\"%s\", v.num != 0.0 ? \"true\" : \"false\");\n\
                 \x20   else printf(\"null\");\n\
                 }\n",
            );
        }
        out.push_str(&c_extra_stdlib(&user_code));
        out.push('\n');
        out.push_str(&top_protos);
        out.push_str(&lambdas);
        out.push_str(&top_fns);
        out.push_str("int main(void) {\n");
        out.push_str(&main_body);
        out.push_str("    return 0;\n}\n");
        return out;
    }

    // Full dynamic runtime included only when arrays, maps, or HOFs are actually used
    out.push_str(
        "typedef enum { ST_NULL, ST_BOOL, ST_NUM, ST_STR, ST_ARR, ST_MAP, ST_FN } SType;\n\
         typedef struct SVal SVal;\n\
         typedef struct SArr SArr;\n\
         typedef struct SMap SMap;\n\
         typedef SVal (*SCFn)(SVal __st_this, int argc, SVal* argv);\n\
         struct SVal { SType type; SMap* env; union { double num; const char* str; SArr* arr; SMap* map; SCFn fn; }; };\n\
         struct SArr { SVal* data; int len; int cap; };\n\
         struct SMap { const char** keys; SVal* vals; int len; int cap; };\n",
    );
    if needs_arena {
        out.push_str(
            "static void** __steel_arena = NULL; static int __steel_arena_len = 0, __steel_arena_cap = 0;\n\
             static void* sv_alloc(size_t sz) {\n\
             \x20   void* p = calloc(1, sz);\n\
             \x20   if (__steel_arena_len >= __steel_arena_cap) {\n\
             \x20       __steel_arena_cap = __steel_arena_cap ? __steel_arena_cap * 2 : 256;\n\
             \x20       __steel_arena = (void**)realloc(__steel_arena, sizeof(void*) * __steel_arena_cap);\n\
             \x20   }\n\
             \x20   __steel_arena[__steel_arena_len++] = p; return p;\n\
             }\n\
             static void sv_cleanup(void) { for (int i = 0; i < __steel_arena_len; i++) free(__steel_arena[i]); free(__steel_arena); }\n",
        );
    }
    out.push_str(
        "static inline SVal sv_null(void) { SVal v; v.type = ST_NULL; v.num = 0; return v; }\n\
         static inline SVal sv_bool(bool b) { SVal v; v.type = ST_BOOL; v.num = b ? 1.0 : 0.0; return v; }\n\
         static inline SVal sv_num(double n) { SVal v; v.type = ST_NUM; v.num = n; return v; }\n\
         static inline SVal sv_str(const char* s) { SVal v; v.type = ST_STR; v.str = s; return v; }\n\
         static inline SVal sv_fn(SCFn f) { SVal v; v.type = ST_FN; v.env = NULL; v.fn = f; return v; }\n\
         static inline SVal sv_call_fn(SVal f, int argc, SVal* argv) {\n\
         \x20   if (f.type != ST_FN || !f.fn) return sv_null();\n\
         \x20   SVal env; env.type = f.env ? ST_MAP : ST_NULL; env.env = NULL; env.map = f.env;\n\
         \x20   return f.fn(env, argc, argv);\n\
         }\n",
    );
    if needs_arr {
        out.push_str(
            "static SVal sv_arr_new(int count, SVal* init_items) {\n\
             \x20   SArr* a = (SArr*)sv_alloc(sizeof(SArr)); a->cap = count > 4 ? count * 2 : 8; a->len = count;\n\
             \x20   a->data = (SVal*)sv_alloc(sizeof(SVal) * a->cap);\n\
             \x20   for (int i = 0; i < count; i++) a->data[i] = init_items[i];\n\
             \x20   SVal v; v.type = ST_ARR; v.arr = a; return v;\n\
             }\n\
             static SVal sv_push(SVal arr, SVal item) {\n\
             \x20   if (arr.type != ST_ARR) return arr;\n\
             \x20   SArr* a = arr.arr;\n\
             \x20   if (a->len >= a->cap) { int nc = a->cap * 2; SVal* nd = (SVal*)sv_alloc(sizeof(SVal) * nc); memcpy(nd, a->data, sizeof(SVal) * a->len); a->data = nd; a->cap = nc; }\n\
             \x20   a->data[a->len++] = item; return arr;\n\
             }\n\
             static SVal sv_pop(SVal arr) { return (arr.type != ST_ARR || arr.arr->len == 0) ? sv_null() : arr.arr->data[--arr.arr->len]; }\n\
             static SVal sv_range(SVal a, SVal b) { int s = (int)a.num, e = (int)b.num; SVal r = sv_arr_new(0, NULL); for (int i = s; i < e; i++) sv_push(r, sv_num(i)); return r; }\n",
        );
    }
    if needs_map {
        out.push_str(
            "static SVal sv_map_new(void) {\n\
             \x20   SMap* m = (SMap*)sv_alloc(sizeof(SMap)); m->cap = 16; m->len = 0;\n\
             \x20   m->keys = (const char**)sv_alloc(sizeof(char*) * m->cap); m->vals = (SVal*)sv_alloc(sizeof(SVal) * m->cap);\n\
             \x20   SVal v; v.type = ST_MAP; v.map = m; return v;\n\
             }\n\
             static void sv_map_set(SVal obj, const char* key, SVal val) {\n\
             \x20   if (obj.type != ST_MAP) return;\n\
             \x20   SMap* m = obj.map;\n\
             \x20   for (int i = 0; i < m->len; i++) if (strcmp(m->keys[i], key) == 0) { m->vals[i] = val; return; }\n\
             \x20   if (m->len >= m->cap) { int nc = m->cap * 2; const char** nk = (const char**)sv_alloc(sizeof(char*) * nc); SVal* nv = (SVal*)sv_alloc(sizeof(SVal) * nc); memcpy(nk, m->keys, sizeof(char*) * m->len); memcpy(nv, m->vals, sizeof(SVal) * m->len); m->keys = nk; m->vals = nv; m->cap = nc; }\n\
             \x20   m->keys[m->len] = key; m->vals[m->len] = val; m->len++;\n\
             }\n\
             static SVal sv_map_get(SVal obj, const char* key) {\n\
             \x20   if (obj.type != ST_MAP) return sv_null();\n\
             \x20   for (int i = 0; i < obj.map->len; i++) if (strcmp(obj.map->keys[i], key) == 0) return obj.map->vals[i];\n\
             \x20   return sv_null();\n\
             }\n\
             static SVal sv_call_method(SVal obj, const char* name, int argc, ...) {\n\
             \x20   SVal m = sv_map_get(obj, name);\n\
             \x20   if (m.type != ST_FN || !m.fn) return sv_null();\n\
             \x20   SVal argv[16]; va_list ap; va_start(ap, argc);\n\
             \x20   for (int i = 0; i < argc && i < 16; i++) argv[i] = va_arg(ap, SVal);\n\
             \x20   va_end(ap); return m.fn(obj, argc, argv);\n\
             }\n\
             static SVal sv_closure(SCFn fn, int ncap, const char** ks, SVal* vs) {\n\
             \x20   SVal env = sv_map_new();\n\
             \x20   for (int i = 0; i < ncap; i++) sv_map_set(env, ks[i], vs[i]);\n\
             \x20   SVal v = sv_fn(fn); v.env = env.map; return v;\n\
             }\n",
        );
    }
    if user_code.contains("sv_get(") || user_code.contains("sv_set(") {
        out.push_str(
            "static SVal sv_get(SVal t, SVal idx) {\n\
             \x20   if (t.type == ST_MAP && idx.type == ST_STR) return sv_map_get(t, idx.str);\n\
             \x20   if (t.type == ST_ARR && idx.type == ST_NUM) { int i = (int)idx.num; if (i < 0) i += t.arr->len; return (i >= 0 && i < t.arr->len) ? t.arr->data[i] : sv_null(); }\n\
             \x20   return sv_null();\n\
             }\n\
             static void sv_set(SVal t, SVal idx, SVal val) { if (t.type == ST_MAP && idx.type == ST_STR) { sv_map_set(t, idx.str, val); return; } if (t.type == ST_ARR && idx.type == ST_NUM) { int i = (int)idx.num; if (i < 0) i += t.arr->len; if (i >= 0 && i < t.arr->len) t.arr->data[i] = val; } }\n",
        );
    }
    out.push_str(
        "static inline bool sv_truthy(SVal v) {\n\
         \x20   if (v.type == ST_NULL) return false;\n\
         \x20   if (v.type == ST_BOOL || v.type == ST_NUM) return v.num != 0.0;\n\
         \x20   if (v.type == ST_STR) return v.str && v.str[0] != '\\0';\n\
         \x20   return true;\n\
         }\n\
         static bool sv_eq(SVal a, SVal b) {\n\
         \x20   if (a.type != b.type) return false;\n\
         \x20   return a.type == ST_STR ? strcmp(a.str, b.str) == 0 : a.num == b.num;\n\
         }\n\
         static SVal sv_str_cast(SVal v) {\n\
         \x20   if (v.type == ST_STR) return v;\n\
         \x20   char* buf = (char*)sv_alloc(128);\n\
         \x20   if (v.type == ST_NULL) snprintf(buf, 128, \"null\");\n\
         \x20   else if (v.type == ST_BOOL) snprintf(buf, 128, \"%s\", v.num != 0.0 ? \"true\" : \"false\");\n\
         \x20   else snprintf(buf, 128, floor(v.num) == v.num ? \"%.0f\" : \"%g\", v.num);\n\
         \x20   return sv_str(buf);\n\
         }\n\
         static SVal sv_add(SVal a, SVal b) {\n\
         \x20   if (a.type == ST_NUM && b.type == ST_NUM) return sv_num(a.num + b.num);\n\
         \x20   SVal sa = sv_str_cast(a), sb = sv_str_cast(b);\n\
         \x20   char* res = (char*)sv_alloc(strlen(sa.str) + strlen(sb.str) + 1);\n\
         \x20   strcpy(res, sa.str); strcat(res, sb.str);\n\
         \x20   return sv_str(res);\n\
         }\n",
    );
    if needs_len {
        out.push_str(
            "static SVal sv_len(SVal v) {\n\
             \x20   if (v.type == ST_ARR) return sv_num(v.arr->len);\n\
             \x20   if (v.type == ST_STR) return sv_num((double)strlen(v.str));\n\
             \x20   if (v.type == ST_MAP) return sv_num(v.map->len);\n\
             \x20   return sv_num(0);\n\
             }\n",
        );
    }
    if needs_hof {
        out.push_str(
            "static SVal sv_map_hof(SVal arr, SVal fn) { SVal out = sv_arr_new(0, NULL); SVal env; env.type = fn.env ? ST_MAP : ST_NULL; env.env = NULL; env.map = fn.env; for (int i = 0; i < arr.arr->len; i++) sv_push(out, fn.fn(env, 1, &arr.arr->data[i])); return out; }\n\
             static SVal sv_filter_hof(SVal arr, SVal fn) { SVal out = sv_arr_new(0, NULL); SVal env; env.type = fn.env ? ST_MAP : ST_NULL; env.env = NULL; env.map = fn.env; for (int i = 0; i < arr.arr->len; i++) if (sv_truthy(fn.fn(env, 1, &arr.arr->data[i]))) sv_push(out, arr.arr->data[i]); return out; }\n\
             static SVal sv_reduce_hof(SVal arr, SVal fn, SVal init) { SVal acc = init; SVal env; env.type = fn.env ? ST_MAP : ST_NULL; env.env = NULL; env.map = fn.env; for (int i = 0; i < arr.arr->len; i++) { SVal a[2] = { acc, arr.arr->data[i] }; acc = fn.fn(env, 2, a); } return acc; }\n",
        );
    }
    out.push_str(
        "static void sv_print_one(SVal v) {\n\
         \x20   if (v.type == ST_ARR) { printf(\"[\"); for (int i = 0; i < v.arr->len; i++) { if (i > 0) printf(\", \"); sv_print_one(v.arr->data[i]); } printf(\"]\"); }\n\
         \x20   else printf(\"%s\", sv_str_cast(v).str);\n\
         }\n\n",
    );

    out.push_str(&c_extra_stdlib(&user_code));
    out.push_str(&top_protos);
    out.push_str(&lambdas);
    out.push_str(&top_fns);
    out.push_str("int main(void) {\n");
    out.push_str(&main_body);
    if needs_arena {
        out.push_str("    sv_cleanup();\n");
    }
    out.push_str("    return 0;\n}\n");
    out
}

/// Attempts to emit 100% pure, human-written C (`const char*`, `double`, `bool`, `printf`)
/// with ZERO `SVal` runtime header whenever a program's types are statically resolvable!
fn try_emit_human_c(program: &Program) -> Option<String> {
    use crate::typecheck::{analyze_program, InferredType};
    let report = analyze_program(program);

    // Check if any statement uses dynamic arrays, maps, classes, or unresolved `Any`
    let mut fn_sigs: std::collections::BTreeMap<String, (Vec<InferredType>, InferredType)> =
        std::collections::BTreeMap::new();

    for stmt in &program.statements {
        match stmt {
            Stmt::ClassDecl { .. } | Stmt::TryCatch { .. } | Stmt::Throw(_) => return None,
            Stmt::FnDecl { name, params, body } => {
                // Infer parameter types from how they are compared/used in the function body
                let mut local_env = report.symbols.clone();
                let mut p_types = Vec::new();
                for (p, def) in params {
                    let guessed = guess_param_type_in_body(p, def.as_ref(), body)?;
                    local_env.insert(p.clone(), guessed.clone());
                    p_types.push(guessed);
                }
                let ret_ty = infer_fn_return_type(body, &local_env)?;
                fn_sigs.insert(name.clone(), (p_types, ret_ty));
            }
            _ => {}
        }
    }

    let mut env: std::collections::BTreeMap<String, InferredType> = std::collections::BTreeMap::new();
    for (k, (_, r)) in &fn_sigs {
        env.insert(
            k.clone(),
            InferredType::Func {
                params: 0,
                ret: Box::new(r.clone()),
            },
        );
    }

    let mut fns_out = String::new();
    let mut main_out = String::new();
    let mut declared = std::collections::HashSet::new();

    for stmt in &program.statements {
        if let Stmt::FnDecl { name, params, body } = stmt {
            let (p_types, ret_ty) = fn_sigs.get(name)?;
            let mut local_env = env.clone();
            let mut local_decl = std::collections::HashSet::new();
            let mut sig_parts = Vec::new();
            for ((p_name, _), p_ty) in params.iter().zip(p_types.iter()) {
                local_env.insert(p_name.clone(), p_ty.clone());
                local_decl.insert(p_name.clone());
                sig_parts.push(format!("{} {}", c_native_type(p_ty)?, p_name));
            }
            fns_out.push_str(&format!(
                "{} {}({}) {{\n",
                c_native_type(ret_ty)?,
                name,
                if sig_parts.is_empty() {
                    "void".to_string()
                } else {
                    sig_parts.join(", ")
                }
            ));
            for st in body {
                fns_out.push_str(&human_c_stmt(
                    st,
                    1,
                    &mut local_env,
                    &mut local_decl,
                    &fn_sigs,
                )?);
            }
            fns_out.push_str("}\n\n");
        } else {
            main_out.push_str(&human_c_stmt(
                stmt,
                1,
                &mut env,
                &mut declared,
                &fn_sigs,
            )?);
        }
    }

    let mut out = String::from("#include <stdio.h>\n#include <string.h>\n#include <stdbool.h>\n");
    if fns_out.contains("pow(") || fns_out.contains("fmod(") || main_out.contains("pow(") || main_out.contains("fmod(") {
        out.push_str("#include <math.h>\n");
    }
    out.push('\n');
    out.push_str(&fns_out);
    out.push_str("int main(void) {\n");
    out.push_str(&main_out);
    out.push_str("    return 0;\n}\n");
    Some(out)
}

fn c_native_type(ty: &crate::typecheck::InferredType) -> Option<&'static str> {
    use crate::typecheck::InferredType;
    match ty {
        InferredType::Num => Some("double"),
        InferredType::Str => Some("const char*"),
        InferredType::Bool => Some("bool"),
        InferredType::Null => Some("void"),
        _ => None,
    }
}

fn guess_param_type_in_body(
    param: &str,
    default: Option<&Expr>,
    body: &[Stmt],
) -> Option<crate::typecheck::InferredType> {
    use crate::typecheck::InferredType;
    if let Some(d) = default {
        match d {
            Expr::Number(_) => return Some(InferredType::Num),
            Expr::String(_) => return Some(InferredType::Str),
            Expr::Bool(_) => return Some(InferredType::Bool),
            _ => {}
        }
    }
    for st in body {
        if let Some(t) = scan_stmt_for_param(param, st) {
            return Some(t);
        }
    }
    Some(InferredType::Num)
}

fn scan_stmt_for_param(
    param: &str,
    stmt: &Stmt,
) -> Option<crate::typecheck::InferredType> {
    match stmt {
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            if let Some(t) = scan_expr_for_param(param, cond) {
                return Some(t);
            }
            for s in then_branch {
                if let Some(t) = scan_stmt_for_param(param, s) {
                    return Some(t);
                }
            }
            if let Some(eb) = else_branch {
                for s in eb {
                    if let Some(t) = scan_stmt_for_param(param, s) {
                        return Some(t);
                    }
                }
            }
            None
        }
        Stmt::Return(Some(e)) | Stmt::Expr(e) | Stmt::Let { value: e, .. } => {
            scan_expr_for_param(param, e)
        }
        Stmt::While { cond, body } => {
            scan_expr_for_param(param, cond).or_else(|| {
                body.iter().find_map(|s| scan_stmt_for_param(param, s))
            })
        }
        _ => None,
    }
}

fn scan_expr_for_param(
    param: &str,
    expr: &Expr,
) -> Option<crate::typecheck::InferredType> {
    use crate::typecheck::InferredType;
    match expr {
        Expr::Binary { left, op, right } => {
            if matches!(&**left, Expr::Ident(n) if n == param) {
                match &**right {
                    Expr::String(_) => return Some(InferredType::Str),
                    Expr::Number(_) => return Some(InferredType::Num),
                    Expr::Bool(_) => return Some(InferredType::Bool),
                    _ => {}
                }
            }
            if matches!(&**right, Expr::Ident(n) if n == param) {
                match &**left {
                    Expr::String(_) => return Some(InferredType::Str),
                    Expr::Number(_) => return Some(InferredType::Num),
                    Expr::Bool(_) => return Some(InferredType::Bool),
                    _ => {}
                }
            }
            if matches!(op, BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod)
                && (matches!(&**left, Expr::Ident(n) if n == param)
                    || matches!(&**right, Expr::Ident(n) if n == param))
            {
                return Some(InferredType::Num);
            }
            scan_expr_for_param(param, left).or_else(|| scan_expr_for_param(param, right))
        }
        _ => None,
    }
}

fn infer_fn_return_type(
    body: &[Stmt],
    env: &std::collections::BTreeMap<String, crate::typecheck::InferredType>,
) -> Option<crate::typecheck::InferredType> {
    use crate::typecheck::{infer_expr, InferredType};
    for st in body {
        match st {
            Stmt::Return(Some(e)) => {
                let t = infer_expr(e, env);
                if matches!(t, InferredType::Num | InferredType::Str | InferredType::Bool) {
                    return Some(t);
                }
                return None;
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if let Some(t) = infer_fn_return_type(then_branch, env) {
                    return Some(t);
                }
                if let Some(eb) = else_branch {
                    if let Some(t) = infer_fn_return_type(eb, env) {
                        return Some(t);
                    }
                }
            }
            _ => {}
        }
    }
    Some(InferredType::Null)
}

fn human_c_stmt(
    stmt: &Stmt,
    lvl: usize,
    env: &mut std::collections::BTreeMap<String, crate::typecheck::InferredType>,
    declared: &mut std::collections::HashSet<String>,
    fn_sigs: &std::collections::BTreeMap<
        String,
        (Vec<crate::typecheck::InferredType>, crate::typecheck::InferredType),
    >,
) -> Option<String> {
    use crate::typecheck::{infer_expr, InferredType};
    let pad = indent(lvl);
    match stmt {
        Stmt::Let { name, value, .. } => {
            let ty = infer_expr(value, env);
            let c_ty = c_native_type(&ty)?;
            let val_s = human_c_expr(value, env, fn_sigs)?;
            env.insert(name.clone(), ty);
            declared.insert(name.clone());
            Some(format!("{}{} {} = {};\n", pad, c_ty, name, val_s))
        }
        Stmt::Assign {
            target: AssignTarget::Ident(name),
            value,
        } => {
            let ty = infer_expr(value, env);
            let c_ty = c_native_type(&ty)?;
            let val_s = human_c_expr(value, env, fn_sigs)?;
            // If user wrote `emotion = emotion(...)` where `emotion` is a function name, rename var to `emotion_val`!
            let var_name = if fn_sigs.contains_key(name) {
                format!("{}_val", name)
            } else {
                name.clone()
            };
            env.insert(name.clone(), ty.clone());
            env.insert(var_name.clone(), ty);
            if !declared.contains(&var_name) {
                declared.insert(var_name.clone());
                Some(format!("{}{} {} = {};\n", pad, c_ty, var_name, val_s))
            } else {
                Some(format!("{}{} = {};\n", pad, var_name, val_s))
            }
        }
        Stmt::Return(Some(e)) => Some(format!("{}return {};\n", pad, human_c_expr(e, env, fn_sigs)?)),
        Stmt::Return(None) => Some(format!("{}return;\n", pad)),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let mut s = format!("{}if ({}) {{\n", pad, human_c_expr(cond, env, fn_sigs)?);
            for st in then_branch {
                s.push_str(&human_c_stmt(st, lvl + 1, env, declared, fn_sigs)?);
            }
            if let Some(eb) = else_branch {
                s.push_str(&format!("{}}} else {{\n", pad));
                for st in eb {
                    s.push_str(&human_c_stmt(st, lvl + 1, env, declared, fn_sigs)?);
                }
            }
            s.push_str(&format!("{}}}\n", pad));
            Some(s)
        }
        Stmt::While { cond, body } => {
            let mut s = format!("{}while ({}) {{\n", pad, human_c_expr(cond, env, fn_sigs)?);
            for st in body {
                s.push_str(&human_c_stmt(st, lvl + 1, env, declared, fn_sigs)?);
            }
            s.push_str(&format!("{}}}\n", pad));
            Some(s)
        }
        Stmt::For {
            var,
            iterable: Expr::Range { start, end },
            body,
        } => {
            let s_val = human_c_expr(start, env, fn_sigs)?;
            let e_val = human_c_expr(end, env, fn_sigs)?;
            env.insert(var.clone(), InferredType::Num);
            declared.insert(var.clone());
            let mut s = format!(
                "{}for (int {} = (int)({}); {} < (int)({}); {}++) {{\n",
                pad, var, s_val, var, e_val, var
            );
            for st in body {
                s.push_str(&human_c_stmt(st, lvl + 1, env, declared, fn_sigs)?);
            }
            s.push_str(&format!("{}}}\n", pad));
            Some(s)
        }
        Stmt::Expr(Expr::Call { callee, args }) => {
            if let Expr::Ident(name) = &**callee {
                if name == "print" || name == "println" {
                    let mut fmt_parts = Vec::new();
                    let mut c_args = Vec::new();
                    for a in args {
                        let ty = infer_expr(a, env);
                        let expr_s = human_c_expr(a, env, fn_sigs)?;
                        match ty {
                            InferredType::Str => {
                                fmt_parts.push("%s");
                                c_args.push(expr_s);
                            }
                            InferredType::Num => {
                                fmt_parts.push("%g");
                                c_args.push(expr_s);
                            }
                            InferredType::Bool => {
                                fmt_parts.push("%s");
                                c_args.push(format!("({} ? \"true\" : \"false\")", expr_s));
                            }
                            _ => return None,
                        }
                    }
                    let fmt_str = format!("{}\\n", fmt_parts.join(" "));
                    if c_args.is_empty() {
                        return Some(format!("{}printf(\"\\n\");\n", pad));
                    } else {
                        return Some(format!(
                            "{}printf(\"{}\", {});\n",
                            pad,
                            fmt_str,
                            c_args.join(", ")
                        ));
                    }
                }
            }
            Some(format!(
                "{}{};\n",
                pad,
                human_c_expr(
                    &Expr::Call {
                        callee: callee.clone(),
                        args: args.clone()
                    },
                    env,
                    fn_sigs
                )?
            ))
        }
        _ => None,
    }
}

fn human_c_expr(
    expr: &Expr,
    env: &std::collections::BTreeMap<String, crate::typecheck::InferredType>,
    fn_sigs: &std::collections::BTreeMap<
        String,
        (Vec<crate::typecheck::InferredType>, crate::typecheck::InferredType),
    >,
) -> Option<String> {
    use crate::typecheck::{infer_expr, InferredType};
    match expr {
        Expr::Number(n) => Some(fmt_num(*n)),
        Expr::String(s) => Some(format!("\"{}\"", escape_str(s))),
        Expr::Bool(b) => Some(b.to_string()),
        Expr::Ident(n) => {
            let actual = if env.contains_key(&format!("{}_val", n)) {
                format!("{}_val", n)
            } else {
                n.clone()
            };
            if env.contains_key(n) || fn_sigs.contains_key(n) {
                Some(actual)
            } else {
                // Unquoted identifier fallback if user wrote `emotion(sad)` instead of `emotion("sad")`
                Some(format!("\"{}\"", escape_str(n)))
            }
        }
        Expr::Unary { op, expr: inner } => {
            let e = human_c_expr(inner, env, fn_sigs)?;
            match op {
                UnaryOp::Neg => Some(format!("(-{})", e)),
                UnaryOp::Not => Some(format!("(!{})", e)),
                UnaryOp::BitNot => Some(format!("(~(long){})", e)),
            }
        }
        Expr::Binary { left, op, right } => {
            let lt = infer_expr(left, env);
            let rt = infer_expr(right, env);
            let l = human_c_expr(left, env, fn_sigs)?;
            let r = human_c_expr(right, env, fn_sigs)?;
            match op {
                BinaryOp::Eq if lt == InferredType::Str || rt == InferredType::Str => {
                    Some(format!("(strcmp({}, {}) == 0)", l, r))
                }
                BinaryOp::Neq if lt == InferredType::Str || rt == InferredType::Str => {
                    Some(format!("(strcmp({}, {}) != 0)", l, r))
                }
                BinaryOp::Add if lt == InferredType::Str || rt == InferredType::Str => None,
                BinaryOp::Add => Some(format!("({} + {})", l, r)),
                BinaryOp::Sub => Some(format!("({} - {})", l, r)),
                BinaryOp::Mul => Some(format!("({} * {})", l, r)),
                BinaryOp::Div => Some(format!("({} / {})", l, r)),
                BinaryOp::Mod => Some(format!("fmod({}, {})", l, r)),
                BinaryOp::Pow => Some(format!("pow({}, {})", l, r)),
                BinaryOp::Eq => Some(format!("({} == {})", l, r)),
                BinaryOp::Neq => Some(format!("({} != {})", l, r)),
                BinaryOp::Lt => Some(format!("({} < {})", l, r)),
                BinaryOp::Gt => Some(format!("({} > {})", l, r)),
                BinaryOp::Lte => Some(format!("({} <= {})", l, r)),
                BinaryOp::Gte => Some(format!("({} >= {})", l, r)),
                BinaryOp::And => Some(format!("({} && {})", l, r)),
                BinaryOp::Or => Some(format!("({} || {})", l, r)),
                _ => None,
            }
        }
        Expr::Ternary {
            cond,
            then_expr,
            else_expr,
        } => {
            let c = human_c_expr(cond, env, fn_sigs)?;
            let t = human_c_expr(then_expr, env, fn_sigs)?;
            let e = human_c_expr(else_expr, env, fn_sigs)?;
            Some(format!("({} ? {} : {})", c, t, e))
        }
        Expr::Call { callee, args } => {
            if let Expr::Ident(name) = &**callee {
                if fn_sigs.contains_key(name) {
                    let mut a_strs = Vec::new();
                    for a in args {
                        a_strs.push(human_c_expr(a, env, fn_sigs)?);
                    }
                    return Some(format!("{}({})", name, a_strs.join(", ")));
                }
            }
            None
        }
        _ => None,
    }
}

fn c_safe_ident(name: &str) -> String {
    match name {
        "this" | "self" => "__st_this".to_string(),
        "auto" | "char" | "const" | "default" | "do" | "double" | "enum" | "extern"
        | "float" | "goto" | "int" | "long" | "register" | "short" | "signed" | "sizeof"
        | "static" | "struct" | "switch" | "typedef" | "union" | "unsigned" | "void"
        | "volatile" | "class" | "delete" | "namespace" | "new" | "operator" | "private"
        | "protected" | "public" | "template" | "typename" | "virtual" => {
            format!("_st_{}", name)
        }
        _ => name.to_string(),
    }
}

thread_local! {
    static C_TOP_FNS: std::cell::RefCell<std::collections::HashSet<String>> = std::cell::RefCell::new(std::collections::HashSet::new());
    static C_CUR_LOCALS: std::cell::RefCell<std::collections::HashSet<String>> = std::cell::RefCell::new(std::collections::HashSet::new());
}

fn c_fn_wrapper(safe_name: &str, arity: usize) -> String {
    let args: Vec<String> = (0..arity)
        .map(|i| format!("(_st_argc > {} ? _st_argv[{}] : sv_null())", i, i))
        .collect();
    format!(
        "SVal __st_fn_{}(SVal __st_this, int _st_argc, SVal* _st_argv) {{ (void)__st_this; (void)_st_argc; (void)_st_argv; return {}({}); }}\n\n",
        safe_name,
        safe_name,
        args.join(", ")
    )
}

fn collect_expr_idents(expr: &Expr, out: &mut std::collections::HashSet<String>) {
    match expr {
        Expr::Ident(n) => { out.insert(c_safe_ident(n)); }
        Expr::Array(xs) => for x in xs { collect_expr_idents(x, out); },
        Expr::Map(es) => for (_, v) in es { collect_expr_idents(v, out); },
        Expr::Unary { expr, .. } | Expr::Spread(expr) => collect_expr_idents(expr, out),
        Expr::Binary { left, right, .. } => { collect_expr_idents(left, out); collect_expr_idents(right, out); }
        Expr::Ternary { cond, then_expr, else_expr } => {
            collect_expr_idents(cond, out); collect_expr_idents(then_expr, out); collect_expr_idents(else_expr, out);
        }
        Expr::Call { callee, args } => {
            collect_expr_idents(callee, out);
            for a in args { collect_expr_idents(a, out); }
        }
        Expr::New { args, .. } => for a in args { collect_expr_idents(a, out); },
        Expr::Index { target, index } => { collect_expr_idents(target, out); collect_expr_idents(index, out); }
        Expr::Member { target, .. } => collect_expr_idents(target, out),
        Expr::Range { start, end } => { collect_expr_idents(start, out); collect_expr_idents(end, out); }
        Expr::FnExpr { body, .. } => for s in body { collect_stmt_idents(s, out); },
        _ => {}
    }
}

fn collect_stmt_idents(stmt: &Stmt, out: &mut std::collections::HashSet<String>) {
    match stmt {
        Stmt::Let { value, .. } => collect_expr_idents(value, out),
        Stmt::Assign { target, value } => {
            match target {
                AssignTarget::Ident(n) => { out.insert(c_safe_ident(n)); }
                AssignTarget::Index { target, index } => { collect_expr_idents(target, out); collect_expr_idents(index, out); }
                AssignTarget::Member { target, .. } => collect_expr_idents(target, out),
            }
            collect_expr_idents(value, out);
        }
        Stmt::Return(Some(e)) | Stmt::Expr(e) | Stmt::Throw(e) => collect_expr_idents(e, out),
        Stmt::If { cond, then_branch, else_branch } => {
            collect_expr_idents(cond, out);
            for s in then_branch { collect_stmt_idents(s, out); }
            if let Some(eb) = else_branch { for s in eb { collect_stmt_idents(s, out); } }
        }
        Stmt::While { cond, body } => {
            collect_expr_idents(cond, out);
            for s in body { collect_stmt_idents(s, out); }
        }
        Stmt::For { iterable, body, .. } => {
            collect_expr_idents(iterable, out);
            for s in body { collect_stmt_idents(s, out); }
        }
        Stmt::TryCatch { try_body, catch_body, .. } => {
            for s in try_body { collect_stmt_idents(s, out); }
            for s in catch_body { collect_stmt_idents(s, out); }
        }
        _ => {}
    }
}

fn c_top_fn(
    name: &str,
    params: &[(String, Option<Expr>)],
    body: &[Stmt],
    lambdas: &mut String,
    lambda_id: &mut usize,
) -> String {
    let safe_name = c_safe_ident(name);
    // Check if function is purely numeric (ideal for hardware-register double fast-path!)
    if is_pure_numeric_fn(name, params, body) {
        let raw_params: Vec<String> = params.iter().map(|(p, _)| format!("double {}", c_safe_ident(p))).collect();
        let sval_params: Vec<String> = params.iter().map(|(p, _)| format!("SVal {}", c_safe_ident(p))).collect();
        let raw_call_args: Vec<String> = params.iter().map(|(p, _)| format!("{}.num", c_safe_ident(p))).collect();

        let mut s = format!(
            "static inline double __raw_{}({}) {{\n",
            safe_name,
            if raw_params.is_empty() {
                "void".to_string()
            } else {
                raw_params.join(", ")
            }
        );
        for st in body {
            s.push_str(&c_raw_num_stmt(st, 1, name));
        }
        s.push_str("    return 0.0;\n}\n");
        s.push_str(&format!(
            "SVal {}({}) {{\n    return sv_num(__raw_{}({}));\n}}\n",
            safe_name,
            if sval_params.is_empty() {
                "void".to_string()
            } else {
                sval_params.join(", ")
            },
            safe_name,
            raw_call_args.join(", ")
        ));
        s.push_str(&c_fn_wrapper(&safe_name, params.len()));
        return s;
    }

    let mut declared = std::collections::HashSet::new();
    let sig: Vec<String> = params
        .iter()
        .map(|(p, _)| {
            let sp = c_safe_ident(p);
            declared.insert(sp.clone());
            format!("SVal {}", sp)
        })
        .collect();
    C_CUR_LOCALS.with(|l| *l.borrow_mut() = declared.clone());
    let mut s = format!(
        "SVal {}({}) {{\n",
        safe_name,
        if sig.is_empty() {
            "void".to_string()
        } else {
            sig.join(", ")
        }
    );
    for st in body {
        s.push_str(&c_stmt(st, 1, &mut declared, lambdas, lambda_id));
        C_CUR_LOCALS.with(|l| *l.borrow_mut() = declared.clone());
    }
    C_CUR_LOCALS.with(|l| l.borrow_mut().clear());
    if !matches!(body.last(), Some(Stmt::Return(_))) {
        s.push_str("    return sv_null();\n");
    }
    s.push_str("}\n");
    s.push_str(&c_fn_wrapper(&safe_name, params.len()));
    s
}

fn is_pure_numeric_fn(fn_name: &str, params: &[(String, Option<Expr>)], body: &[Stmt]) -> bool {
    if params.is_empty() || body.is_empty() {
        return false;
    }
    let mut locals: std::collections::HashSet<String> =
        params.iter().map(|(p, _)| p.clone()).collect();
    let mut has_return = false;
    for st in body {
        if !check_numeric_stmt(st, fn_name, &mut locals, &mut has_return) {
            return false;
        }
    }
    has_return
}

fn check_numeric_stmt(
    stmt: &Stmt,
    fn_name: &str,
    locals: &mut std::collections::HashSet<String>,
    has_return: &mut bool,
) -> bool {
    match stmt {
        Stmt::Let { name, value, .. } => {
            if check_numeric_expr(value, fn_name, locals) {
                locals.insert(name.clone());
                true
            } else {
                false
            }
        }
        Stmt::Assign {
            target: AssignTarget::Ident(name),
            value,
        } => locals.contains(name) && check_numeric_expr(value, fn_name, locals),
        Stmt::Return(Some(e)) => {
            *has_return = true;
            check_numeric_expr(e, fn_name, locals)
        }
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            if !check_numeric_expr(cond, fn_name, locals) {
                return false;
            }
            for s in then_branch {
                if !check_numeric_stmt(s, fn_name, locals, has_return) {
                    return false;
                }
            }
            if let Some(eb) = else_branch {
                for s in eb {
                    if !check_numeric_stmt(s, fn_name, locals, has_return) {
                        return false;
                    }
                }
            }
            true
        }
        Stmt::While { cond, body } => {
            if !check_numeric_expr(cond, fn_name, locals) {
                return false;
            }
            for s in body {
                if !check_numeric_stmt(s, fn_name, locals, has_return) {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}

fn check_numeric_expr(
    expr: &Expr,
    fn_name: &str,
    locals: &std::collections::HashSet<String>,
) -> bool {
    match expr {
        Expr::Number(_) => true,
        Expr::Ident(n) => locals.contains(n),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr: inner,
        } => check_numeric_expr(inner, fn_name, locals),
        Expr::Binary { left, op, right } => {
            matches!(
                op,
                BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Mod
                    | BinaryOp::Pow
                    | BinaryOp::Lt
                    | BinaryOp::Gt
                    | BinaryOp::Lte
                    | BinaryOp::Gte
                    | BinaryOp::Eq
                    | BinaryOp::Neq
                    | BinaryOp::And
                    | BinaryOp::Or
            ) && check_numeric_expr(left, fn_name, locals)
                && check_numeric_expr(right, fn_name, locals)
        }
        Expr::Call { callee, args } => {
            if let Expr::Ident(c) = &**callee {
                if c == fn_name
                    || matches!(
                        c.as_str(),
                        "sqrt" | "sin" | "cos" | "floor" | "ceil" | "round" | "abs"
                    )
                {
                    return args.iter().all(|a| check_numeric_expr(a, fn_name, locals));
                }
            }
            false
        }
        _ => false,
    }
}

fn c_raw_num_stmt(stmt: &Stmt, lvl: usize, fn_name: &str) -> String {
    let pad = indent(lvl);
    match stmt {
        Stmt::Let { name, value, .. } => {
            format!("{}double {} = {};\n", pad, name, c_raw_num_expr(value, fn_name))
        }
        Stmt::Assign {
            target: AssignTarget::Ident(name),
            value,
        } => format!("{}{} = {};\n", pad, name, c_raw_num_expr(value, fn_name)),
        Stmt::Return(Some(e)) => format!("{}return {};\n", pad, c_raw_num_expr(e, fn_name)),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let mut s = format!("{}if ({}) {{\n", pad, c_raw_num_expr(cond, fn_name));
            for st in then_branch {
                s.push_str(&c_raw_num_stmt(st, lvl + 1, fn_name));
            }
            if let Some(eb) = else_branch {
                s.push_str(&format!("{}}} else {{\n", pad));
                for st in eb {
                    s.push_str(&c_raw_num_stmt(st, lvl + 1, fn_name));
                }
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::While { cond, body } => {
            let mut s = format!("{}while ({}) {{\n", pad, c_raw_num_expr(cond, fn_name));
            for st in body {
                s.push_str(&c_raw_num_stmt(st, lvl + 1, fn_name));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        _ => String::new(),
    }
}

fn c_raw_num_expr(expr: &Expr, fn_name: &str) -> String {
    match expr {
        Expr::Number(n) => fmt_num(*n),
        Expr::Ident(n) => n.clone(),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr: inner,
        } => format!("(-{})", c_raw_num_expr(inner, fn_name)),
        Expr::Binary { left, op, right } => {
            let l = c_raw_num_expr(left, fn_name);
            let r = c_raw_num_expr(right, fn_name);
            match op {
                BinaryOp::Add => format!("({} + {})", l, r),
                BinaryOp::Sub => format!("({} - {})", l, r),
                BinaryOp::Mul => format!("({} * {})", l, r),
                BinaryOp::Div => format!("({} / {})", l, r),
                BinaryOp::Mod => format!("fmod({}, {})", l, r),
                BinaryOp::Pow => format!("pow({}, {})", l, r),
                BinaryOp::Lt => format!("({} < {})", l, r),
                BinaryOp::Gt => format!("({} > {})", l, r),
                BinaryOp::Lte => format!("({} <= {})", l, r),
                BinaryOp::Gte => format!("({} >= {})", l, r),
                BinaryOp::Eq => format!("({} == {})", l, r),
                BinaryOp::Neq => format!("({} != {})", l, r),
                BinaryOp::And => format!("({} && {})", l, r),
                BinaryOp::Or => format!("({} || {})", l, r),
                _ => format!("({} + {})", l, r),
            }
        }
        Expr::Call { callee, args } => {
            let arg_strs: Vec<String> = args.iter().map(|a| c_raw_num_expr(a, fn_name)).collect();
            if let Expr::Ident(c) = &**callee {
                if c == fn_name {
                    return format!("__raw_{}({})", fn_name, arg_strs.join(", "));
                }
                if c == "abs" {
                    return format!("fabs({})", arg_strs.join(", "));
                }
                return format!("{}({})", c, arg_strs.join(", "));
            }
            "0.0".to_string()
        }
        _ => "0.0".to_string(),
    }
}

/// Emits an unboxed C boolean condition (`bool`) directly when possible, avoiding
/// redundant `sv_truthy(sv_bool(...))` boxing/unboxing layers!
fn c_cond_expr(expr: &Expr, lambdas: &mut String, lambda_id: &mut usize) -> String {
    match expr {
        Expr::Binary { left, op, right } => match op {
            BinaryOp::Lt => format!(
                "({} < {})",
                c_num_expr(left, lambdas, lambda_id),
                c_num_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::Gt => format!(
                "({} > {})",
                c_num_expr(left, lambdas, lambda_id),
                c_num_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::Lte => format!(
                "({} <= {})",
                c_num_expr(left, lambdas, lambda_id),
                c_num_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::Gte => format!(
                "({} >= {})",
                c_num_expr(left, lambdas, lambda_id),
                c_num_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::Eq => format!(
                "sv_eq({}, {})",
                c_expr(left, lambdas, lambda_id),
                c_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::Neq => format!(
                "(!sv_eq({}, {}))",
                c_expr(left, lambdas, lambda_id),
                c_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::And => format!(
                "({} && {})",
                c_cond_expr(left, lambdas, lambda_id),
                c_cond_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::Or => format!(
                "({} || {})",
                c_cond_expr(left, lambdas, lambda_id),
                c_cond_expr(right, lambdas, lambda_id)
            ),
            _ => format!("sv_truthy({})", c_expr(expr, lambdas, lambda_id)),
        },
        Expr::Unary {
            op: UnaryOp::Not,
            expr: inner,
        } => format!("(!{})", c_cond_expr(inner, lambdas, lambda_id)),
        Expr::Bool(b) => b.to_string(),
        _ => format!("sv_truthy({})", c_expr(expr, lambdas, lambda_id)),
    }
}

/// Emits an unboxed C `double` expression directly for arithmetic chains, eliminating
/// intermediate `sv_num(...)` struct wrapping/unwrapping inside math formulas!
fn c_num_expr(expr: &Expr, lambdas: &mut String, lambda_id: &mut usize) -> String {
    match expr {
        Expr::Number(n) => fmt_num(*n),
        Expr::Ident(name) => match name.as_str() {
            "PI" => "3.14159265358979323846".to_string(),
            "E" => "2.71828182845904523536".to_string(),
            _ => format!("{}.num", c_safe_ident(name)),
        },
        Expr::Unary {
            op: UnaryOp::Neg,
            expr: inner,
        } => format!("(-{})", c_num_expr(inner, lambdas, lambda_id)),
        Expr::Binary { left, op, right } => match op {
            BinaryOp::Add if !self_might_be_string(left) && !self_might_be_string(right) => {
                format!(
                    "({} + {})",
                    c_num_expr(left, lambdas, lambda_id),
                    c_num_expr(right, lambdas, lambda_id)
                )
            }
            BinaryOp::Sub => format!(
                "({} - {})",
                c_num_expr(left, lambdas, lambda_id),
                c_num_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::Mul => format!(
                "({} * {})",
                c_num_expr(left, lambdas, lambda_id),
                c_num_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::Div => format!(
                "({} / {})",
                c_num_expr(left, lambdas, lambda_id),
                c_num_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::Mod => format!(
                "fmod({}, {})",
                c_num_expr(left, lambdas, lambda_id),
                c_num_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::Pow => format!(
                "pow({}, {})",
                c_num_expr(left, lambdas, lambda_id),
                c_num_expr(right, lambdas, lambda_id)
            ),
            _ => format!("({}).num", c_expr(expr, lambdas, lambda_id)),
        },
        _ => format!("({}).num", c_expr(expr, lambdas, lambda_id)),
    }
}

fn self_might_be_string(expr: &Expr) -> bool {
    match expr {
        Expr::String(_) => true,
        Expr::Call { callee, .. } => {
            matches!(&**callee, Expr::Ident(n) if n == "str" || n == "upper" || n == "lower" || n == "join" || n == "trim")
        }
        Expr::Binary { left, op: BinaryOp::Add, right } => {
            self_might_be_string(left) || self_might_be_string(right)
        }
        _ => false,
    }
}

fn c_stmt(
    stmt: &Stmt,
    lvl: usize,
    declared: &mut std::collections::HashSet<String>,
    lambdas: &mut String,
    lambda_id: &mut usize,
) -> String {
    let pad = indent(lvl);
    match stmt {
        Stmt::Import(p) => format!("{}/* import \"{}\" */\n", pad, escape_str(p)),
        Stmt::ClassDecl {
            name,
            parent,
            methods,
        } => c_stmt(
            &desugar_class(name, parent, methods),
            lvl,
            declared,
            lambdas,
            lambda_id,
        ),
        Stmt::TryCatch {
            try_body,
            err_var,
            catch_body,
        } => {
            let mut s = format!("{}if (setjmp(__steel_jmp[__steel_try_sp++]) == 0) {{\n", pad);
            for st in try_body {
                s.push_str(&c_stmt(st, lvl + 1, declared, lambdas, lambda_id));
            }
            s.push_str(&format!("{}    __steel_try_sp--;\n{}}} else {{\n{}    SVal {} = __steel_err;\n", pad, pad, pad, c_safe_ident(err_var)));
            declared.insert(c_safe_ident(err_var));
            for st in catch_body {
                s.push_str(&c_stmt(st, lvl + 1, declared, lambdas, lambda_id));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::Throw(e) => format!(
            "{}{{ __steel_err = {}; if (__steel_try_sp > 0) longjmp(__steel_jmp[--__steel_try_sp], 1); fprintf(stderr, \"Uncaught error: %s\\n\", sv_str_cast(__steel_err).str); exit(1); }}\n",
            pad,
            c_expr(e, lambdas, lambda_id)
        ),
        Stmt::Test { name, body } => {
            let mut s = format!("{}/* test \"{}\" */ {{\n", pad, escape_str(name));
            for st in body {
                s.push_str(&c_stmt(st, lvl + 1, declared, lambdas, lambda_id));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::Defer(inner) => c_stmt(inner, lvl, declared, lambdas, lambda_id),
        Stmt::Let { name, value, .. } => {
            let cname = c_safe_ident(name);
            declared.insert(cname.clone());
            format!(
                "{}SVal {} = {};\n",
                pad,
                cname,
                c_expr(value, lambdas, lambda_id)
            )
        }
        Stmt::Assign { target, value } => match target {
            AssignTarget::Ident(n) => {
                let cname = c_safe_ident(n);
                if !declared.contains(&cname) {
                    declared.insert(cname.clone());
                    format!("{}SVal {} = {};\n", pad, cname, c_expr(value, lambdas, lambda_id))
                } else {
                    format!("{}{} = {};\n", pad, cname, c_expr(value, lambdas, lambda_id))
                }
            }
            AssignTarget::Index { target, index } => format!(
                "{}sv_set({}, {}, {});\n",
                pad,
                c_expr(target, lambdas, lambda_id),
                c_expr(index, lambdas, lambda_id),
                c_expr(value, lambdas, lambda_id)
            ),
            AssignTarget::Member { target, field } => format!(
                "{}sv_map_set({}, \"{}\", {});\n",
                pad,
                c_expr(target, lambdas, lambda_id),
                escape_str(field),
                c_expr(value, lambdas, lambda_id)
            ),
        },
        Stmt::FnDecl { name, params, body } => c_top_fn(name, params, body, lambdas, lambda_id),
        Stmt::Return(expr) => match expr {
            Some(e) => format!("{}return {};\n", pad, c_expr(e, lambdas, lambda_id)),
            None => format!("{}return sv_null();\n", pad),
        },
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let mut s = format!("{}if ({}) {{\n", pad, c_cond_expr(cond, lambdas, lambda_id));
            for st in then_branch {
                s.push_str(&c_stmt(st, lvl + 1, declared, lambdas, lambda_id));
            }
            if let Some(eb) = else_branch {
                s.push_str(&format!("{}}} else {{\n", pad));
                for st in eb {
                    s.push_str(&c_stmt(st, lvl + 1, declared, lambdas, lambda_id));
                }
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::While { cond, body } => {
            let mut s = format!(
                "{}while ({}) {{\n",
                pad,
                c_cond_expr(cond, lambdas, lambda_id)
            );
            for st in body {
                s.push_str(&c_stmt(st, lvl + 1, declared, lambdas, lambda_id));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::For {
            var,
            iterable,
            body,
        } => {
            declared.insert(var.clone());
            // Zero-allocation fast path for `for i in start..end`!
            if let Expr::Range { start, end } = iterable {
                let idx_var = format!("__i_{}", var);
                let end_var = format!("__end_{}", var);
                let mut s = format!(
                    "{}for (int {} = (int)({}), {} = (int)({}); {} < {}; {}++) {{\n",
                    pad,
                    idx_var,
                    c_num_expr(start, lambdas, lambda_id),
                    end_var,
                    c_num_expr(end, lambdas, lambda_id),
                    idx_var,
                    end_var,
                    idx_var
                );
                s.push_str(&format!("{}    SVal {} = sv_num({});\n", pad, var, idx_var));
                for st in body {
                    s.push_str(&c_stmt(st, lvl + 1, declared, lambdas, lambda_id));
                }
                s.push_str(&format!("{}}}\n", pad));
                return s;
            }

            let it_var = format!("__it_{}", var);
            let idx_var = format!("__i_{}", var);
            let mut s = format!(
                "{}SVal {} = {};\n",
                pad,
                it_var,
                c_expr(iterable, lambdas, lambda_id)
            );
            s.push_str(&format!(
                "{}for (int {} = 0; {} < {}.arr->len; {}++) {{\n",
                pad, idx_var, idx_var, it_var, idx_var
            ));
            s.push_str(&format!(
                "{}    SVal {} = {}.arr->data[{}];\n",
                pad, var, it_var, idx_var
            ));
            for st in body {
                s.push_str(&c_stmt(st, lvl + 1, declared, lambdas, lambda_id));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::Break => format!("{}break;\n", pad),
        Stmt::Continue => format!("{}continue;\n", pad),
        Stmt::Expr(e) => {
            if let Expr::Call { callee, args } = e {
                if let Expr::Ident(name) = &**callee {
                    if name == "print" || name == "println" {
                        let mut s = pad.clone();
                        for (i, a) in args.iter().enumerate() {
                            if i > 0 {
                                s.push_str("printf(\" \"); ");
                            }
                            s.push_str(&format!("sv_print_one({}); ", c_expr(a, lambdas, lambda_id)));
                        }
                        s.push_str("printf(\"\\n\");\n");
                        return s;
                    }
                }
            }
            format!("{}{};\n", pad, c_expr(e, lambdas, lambda_id))
        }
    }
}

fn c_expr(expr: &Expr, lambdas: &mut String, lambda_id: &mut usize) -> String {
    match expr {
        Expr::Number(n) => format!("sv_num({})", fmt_num(*n)),
        Expr::String(s) => format!("sv_str(\"{}\")", escape_str(s)),
        Expr::Bool(b) => format!("sv_bool({})", b),
        Expr::Null => "sv_null()".to_string(),
        Expr::Ident(name) => match name.as_str() {
            "PI" => "sv_num(3.141592653589793)".to_string(),
            "E" => "sv_num(2.718281828459045)".to_string(),
            _ => {
                let sn = c_safe_ident(name);
                let is_top = C_TOP_FNS.with(|f| f.borrow().contains(name));
                let is_local = C_CUR_LOCALS.with(|l| l.borrow().contains(&sn));
                if is_top && !is_local {
                    format!("sv_fn(__st_fn_{})", sn)
                } else {
                    sn
                }
            }
        },
        Expr::Array(items) => {
            if items.is_empty() {
                "sv_arr_new(0, NULL)".to_string()
            } else {
                let inner: Vec<String> =
                    items.iter().map(|e| c_expr(e, lambdas, lambda_id)).collect();
                format!(
                    "sv_arr_new({}, (SVal[]){{{}}})",
                    items.len(),
                    inner.join(", ")
                )
            }
        }
        Expr::Map(entries) => {
            if entries.is_empty() {
                "sv_map_new()".to_string()
            } else {
                let mut s = String::from("({ SVal __m = sv_map_new(); ");
                for (k, v) in entries {
                    s.push_str(&format!(
                        "sv_map_set(__m, \"{}\", {}); ",
                        escape_str(k),
                        c_expr(v, lambdas, lambda_id)
                    ));
                }
                s.push_str("__m; })");
                s
            }
        }
        Expr::New { class_name, args } => {
            let arg_strs: Vec<String> =
                args.iter().map(|a| c_expr(a, lambdas, lambda_id)).collect();
            format!("{}({})", class_name, arg_strs.join(", "))
        }
        Expr::Ternary {
            cond,
            then_expr,
            else_expr,
        } => format!(
            "({} ? ({}) : ({}))",
            c_cond_expr(cond, lambdas, lambda_id),
            c_expr(then_expr, lambdas, lambda_id),
            c_expr(else_expr, lambdas, lambda_id)
        ),
        Expr::Binary { left, op, right } => match op {
            BinaryOp::Add => format!(
                "sv_add({}, {})",
                c_expr(left, lambdas, lambda_id),
                c_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod | BinaryOp::Pow => {
                format!("sv_num({})", c_num_expr(expr, lambdas, lambda_id))
            }
            BinaryOp::BitAnd => format!(
                "sv_num((long){} & (long){})",
                c_num_expr(left, lambdas, lambda_id),
                c_num_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::BitOr => format!(
                "sv_num((long){} | (long){})",
                c_num_expr(left, lambdas, lambda_id),
                c_num_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::BitXor => format!(
                "sv_num((long){} ^ (long){})",
                c_num_expr(left, lambdas, lambda_id),
                c_num_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::Shl => format!(
                "sv_num((long){} << (long){})",
                c_num_expr(left, lambdas, lambda_id),
                c_num_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::Shr => format!(
                "sv_num((long){} >> (long){})",
                c_num_expr(left, lambdas, lambda_id),
                c_num_expr(right, lambdas, lambda_id)
            ),
            BinaryOp::NullCoalesce => {
                let l = c_expr(left, lambdas, lambda_id);
                let r = c_expr(right, lambdas, lambda_id);
                format!("(({}).type != ST_NULL ? ({}) : ({}))", l, l, r)
            }
            BinaryOp::Eq
            | BinaryOp::Neq
            | BinaryOp::Lt
            | BinaryOp::Gt
            | BinaryOp::Lte
            | BinaryOp::Gte
            | BinaryOp::And
            | BinaryOp::Or => format!("sv_bool({})", c_cond_expr(expr, lambdas, lambda_id)),
        },
        Expr::Unary { op, expr: inner } => match op {
            UnaryOp::Neg => format!("sv_num(-{})", c_num_expr(inner, lambdas, lambda_id)),
            UnaryOp::Not => format!("sv_bool(!{})", c_cond_expr(inner, lambdas, lambda_id)),
            UnaryOp::BitNot => format!("sv_num(~(long){})", c_num_expr(inner, lambdas, lambda_id)),
        },
        Expr::Call { callee, args } => {
            if let Expr::Ident(name) = &**callee {
                match name.as_str() {
                    "str" => {
                        return format!(
                            "sv_str_cast({})",
                            args.first()
                                .map(|a| c_expr(a, lambdas, lambda_id))
                                .unwrap_or_else(|| "sv_null()".to_string())
                        )
                    }
                    "len" => {
                        return format!(
                            "sv_len({})",
                            args.first()
                                .map(|a| c_expr(a, lambdas, lambda_id))
                                .unwrap_or_else(|| "sv_null()".to_string())
                        )
                    }
                    "num" if args.len() == 1 => {
                        return format!("sv_num_cast({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "push" if args.len() == 2 => {
                        return format!(
                            "sv_push({}, {})",
                            c_expr(&args[0], lambdas, lambda_id),
                            c_expr(&args[1], lambdas, lambda_id)
                        )
                    }
                    "pop" if args.len() == 1 => {
                        return format!("sv_pop({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "map" if args.len() == 2 => {
                        return format!(
                            "sv_map_hof({}, {})",
                            c_expr(&args[0], lambdas, lambda_id),
                            c_expr(&args[1], lambdas, lambda_id)
                        )
                    }
                    "filter" if args.len() == 2 => {
                        return format!(
                            "sv_filter_hof({}, {})",
                            c_expr(&args[0], lambdas, lambda_id),
                            c_expr(&args[1], lambdas, lambda_id)
                        )
                    }
                    "reduce" if args.len() >= 2 => {
                        let init = args
                            .get(2)
                            .map(|a| c_expr(a, lambdas, lambda_id))
                            .unwrap_or_else(|| "sv_num(0)".to_string());
                        return format!(
                            "sv_reduce_hof({}, {}, {})",
                            c_expr(&args[0], lambdas, lambda_id),
                            c_expr(&args[1], lambdas, lambda_id),
                            init
                        );
                    }
                    "sqrt" | "sin" | "cos" | "tan" | "floor" | "ceil" | "round" | "fabs"
                        if args.len() == 1 =>
                    {
                        return format!(
                            "sv_num({}({}))",
                            name,
                            c_num_expr(&args[0], lambdas, lambda_id)
                        )
                    }
                    "pow" if args.len() == 2 => {
                        return format!(
                            "sv_num(pow({}, {}))",
                            c_num_expr(&args[0], lambdas, lambda_id),
                            c_num_expr(&args[1], lambdas, lambda_id)
                        )
                    }
                    "min" if args.len() == 2 => {
                        return format!(
                            "sv_num(fmin({}, {}))",
                            c_num_expr(&args[0], lambdas, lambda_id),
                            c_num_expr(&args[1], lambdas, lambda_id)
                        )
                    }
                    "max" if args.len() == 2 => {
                        return format!(
                            "sv_num(fmax({}, {}))",
                            c_num_expr(&args[0], lambdas, lambda_id),
                            c_num_expr(&args[1], lambdas, lambda_id)
                        )
                    }
                    "abs" if args.len() == 1 => {
                        return format!(
                            "sv_num(fabs({}))",
                            c_num_expr(&args[0], lambdas, lambda_id)
                        )
                    }
                    "read_file" if args.len() == 1 => {
                        return format!("sv_read_file({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "write_file" if args.len() == 2 => {
                        return format!(
                            "sv_write_file({}, {})",
                            c_expr(&args[0], lambdas, lambda_id),
                            c_expr(&args[1], lambdas, lambda_id)
                        )
                    }
                    "file_exists" if args.len() == 1 => {
                        return format!("sv_file_exists({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "env" if args.len() == 1 => {
                        return format!("sv_env({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "upper" if args.len() == 1 => {
                        return format!("sv_upper({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "lower" if args.len() == 1 => {
                        return format!("sv_lower({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "trim" if args.len() == 1 => {
                        return format!("sv_trim({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "contains" if args.len() == 2 => {
                        return format!(
                            "sv_contains({}, {})",
                            c_expr(&args[0], lambdas, lambda_id),
                            c_expr(&args[1], lambdas, lambda_id)
                        )
                    }
                    "starts_with" if args.len() == 2 => {
                        return format!(
                            "sv_starts_with({}, {})",
                            c_expr(&args[0], lambdas, lambda_id),
                            c_expr(&args[1], lambdas, lambda_id)
                        )
                    }
                    "ends_with" if args.len() == 2 => {
                        return format!(
                            "sv_ends_with({}, {})",
                            c_expr(&args[0], lambdas, lambda_id),
                            c_expr(&args[1], lambdas, lambda_id)
                        )
                    }
                    "split" if args.len() == 2 => {
                        return format!(
                            "sv_split({}, {})",
                            c_expr(&args[0], lambdas, lambda_id),
                            c_expr(&args[1], lambdas, lambda_id)
                        )
                    }
                    "join" if args.len() == 2 => {
                        return format!(
                            "sv_join({}, {})",
                            c_expr(&args[0], lambdas, lambda_id),
                            c_expr(&args[1], lambdas, lambda_id)
                        )
                    }
                    "replace" if args.len() == 3 => {
                        return format!(
                            "sv_replace({}, {}, {})",
                            c_expr(&args[0], lambdas, lambda_id),
                            c_expr(&args[1], lambdas, lambda_id),
                            c_expr(&args[2], lambdas, lambda_id)
                        )
                    }
                    "reverse" if args.len() == 1 => {
                        return format!("sv_reverse({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "keys" if args.len() == 1 => {
                        return format!("sv_keys({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "values" if args.len() == 1 => {
                        return format!("sv_values({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "slice" if args.len() >= 2 => {
                        let e = args.get(2).map(|a| c_expr(a, lambdas, lambda_id)).unwrap_or_else(|| "sv_null()".to_string());
                        return format!("sv_slice({}, {}, {})", c_expr(&args[0], lambdas, lambda_id), c_expr(&args[1], lambdas, lambda_id), e);
                    }
                    "sort" if args.len() == 1 => {
                        return format!("sv_sort({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "int" if args.len() == 1 => {
                        return format!("sv_int_cast({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "bool" if args.len() == 1 => {
                        return format!("sv_bool(sv_truthy({}))", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "type" if args.len() == 1 => {
                        return format!("sv_type({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "assert" if !args.is_empty() => {
                        let m = args.get(1).map(|a| c_expr(a, lambdas, lambda_id)).unwrap_or_else(|| "sv_str(\"Assertion failed\")".to_string());
                        return format!("sv_assert({}, {})", c_expr(&args[0], lambdas, lambda_id), m);
                    }
                    "has_key" if args.len() == 2 => {
                        return format!("sv_has_key({}, {})", c_expr(&args[0], lambdas, lambda_id), c_expr(&args[1], lambdas, lambda_id))
                    }
                    "delete_key" if args.len() == 2 => {
                        return format!("sv_delete_key({}, {})", c_expr(&args[0], lambdas, lambda_id), c_expr(&args[1], lambdas, lambda_id))
                    }
                    "concat" if args.len() == 2 => {
                        return format!("sv_concat({}, {})", c_expr(&args[0], lambdas, lambda_id), c_expr(&args[1], lambdas, lambda_id))
                    }
                    "index_of" if args.len() == 2 => {
                        return format!("sv_index_of({}, {})", c_expr(&args[0], lambdas, lambda_id), c_expr(&args[1], lambdas, lambda_id))
                    }
                    "repeat" if args.len() == 2 => {
                        return format!("sv_repeat({}, {})", c_expr(&args[0], lambdas, lambda_id), c_expr(&args[1], lambdas, lambda_id))
                    }
                    "pad_start" if args.len() >= 2 => {
                        let ch = args.get(2).map(|a| c_expr(a, lambdas, lambda_id)).unwrap_or_else(|| "sv_str(\" \")".to_string());
                        return format!("sv_pad_start({}, {}, {})", c_expr(&args[0], lambdas, lambda_id), c_expr(&args[1], lambdas, lambda_id), ch);
                    }
                    "pad_end" if args.len() >= 2 => {
                        let ch = args.get(2).map(|a| c_expr(a, lambdas, lambda_id)).unwrap_or_else(|| "sv_str(\" \")".to_string());
                        return format!("sv_pad_end({}, {}, {})", c_expr(&args[0], lambdas, lambda_id), c_expr(&args[1], lambdas, lambda_id), ch);
                    }
                    "char_code" if args.len() == 1 => {
                        return format!("sv_char_code({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "from_char_code" if args.len() == 1 => {
                        return format!("sv_from_char_code({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "find" if args.len() == 2 => {
                        return format!("sv_find_hof({}, {})", c_expr(&args[0], lambdas, lambda_id), c_expr(&args[1], lambdas, lambda_id))
                    }
                    "any" if args.len() == 2 => {
                        return format!("sv_any_hof({}, {})", c_expr(&args[0], lambdas, lambda_id), c_expr(&args[1], lambdas, lambda_id))
                    }
                    "all" if args.len() == 2 => {
                        return format!("sv_all_hof({}, {})", c_expr(&args[0], lambdas, lambda_id), c_expr(&args[1], lambdas, lambda_id))
                    }
                    "sum" if args.len() == 1 => {
                        return format!("sv_sum({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "avg" if args.len() == 1 => {
                        return format!("sv_avg({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "unique" if args.len() == 1 => {
                        return format!("sv_unique({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "flat" if args.len() == 1 => {
                        return format!("sv_flat({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "zip" if args.len() == 2 => {
                        return format!("sv_zip({}, {})", c_expr(&args[0], lambdas, lambda_id), c_expr(&args[1], lambdas, lambda_id))
                    }
                    "enumerate" if args.len() == 1 => {
                        return format!("sv_enumerate({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "clamp" if args.len() == 3 => {
                        return format!(
                            "sv_num(fmin(fmax({}, {}), {}))",
                            c_num_expr(&args[0], lambdas, lambda_id),
                            c_num_expr(&args[1], lambdas, lambda_id),
                            c_num_expr(&args[2], lambdas, lambda_id)
                        )
                    }
                    "sign" if args.len() == 1 => {
                        let x = c_num_expr(&args[0], lambdas, lambda_id);
                        return format!("sv_num((({}) > 0.0) ? 1.0 : ((({}) < 0.0) ? -1.0 : 0.0))", x, x);
                    }
                    "atan2" if args.len() == 2 => {
                        return format!(
                            "sv_num(atan2({}, {}))",
                            c_num_expr(&args[0], lambdas, lambda_id),
                            c_num_expr(&args[1], lambdas, lambda_id)
                        )
                    }
                    "log" if args.len() == 1 => {
                        return format!("sv_num(log({}))", c_num_expr(&args[0], lambdas, lambda_id))
                    }
                    "exp" if args.len() == 1 => {
                        return format!("sv_num(exp({}))", c_num_expr(&args[0], lambdas, lambda_id))
                    }
                    "json_stringify" if args.len() == 1 => {
                        return format!("sv_json_stringify({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "json_parse" if args.len() == 1 => {
                        return format!("sv_json_parse({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "hash" if args.len() == 1 => {
                        return format!("sv_hash({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "append_file" if args.len() == 2 => {
                        return format!("sv_append_file({}, {})", c_expr(&args[0], lambdas, lambda_id), c_expr(&args[1], lambdas, lambda_id))
                    }
                    "remove_file" if args.len() == 1 => {
                        return format!("sv_remove_file({})", c_expr(&args[0], lambdas, lambda_id))
                    }
                    "clock" if args.is_empty() => {
                        return "sv_clock()".to_string()
                    }
                    "range" => {
                        let a = args
                            .first()
                            .map(|a| c_expr(a, lambdas, lambda_id))
                            .unwrap_or_else(|| "sv_num(0)".to_string());
                        let b = args
                            .get(1)
                            .map(|a| c_expr(a, lambdas, lambda_id))
                            .unwrap_or_else(|| "sv_num(0)".to_string());
                        if let Some(st) = args.get(2) {
                            return format!("sv_range_step({}, {}, {})", a, b, c_expr(st, lambdas, lambda_id));
                        }
                        return format!("sv_range({}, {})", a, b);
                    }
                    _ => {}
                }
            }
            if let Expr::Member { target, field } = &**callee {
                let obj_s = c_expr(target, lambdas, lambda_id);
                let arg_strs: Vec<String> =
                    args.iter().map(|a| c_expr(a, lambdas, lambda_id)).collect();
                if arg_strs.is_empty() {
                    return format!(
                        "sv_call_method({}, \"{}\", 0)",
                        obj_s,
                        escape_str(field)
                    );
                } else {
                    return format!(
                        "sv_call_method({}, \"{}\", {}, {})",
                        obj_s,
                        escape_str(field),
                        arg_strs.len(),
                        arg_strs.join(", ")
                    );
                }
            }
            let arg_strs: Vec<String> =
                args.iter().map(|a| c_expr(a, lambdas, lambda_id)).collect();
            if let Expr::Ident(name) = &**callee {
                let sn = c_safe_ident(name);
                let is_top = C_TOP_FNS.with(|f| f.borrow().contains(name));
                let is_local = C_CUR_LOCALS.with(|l| l.borrow().contains(&sn));
                if is_top && !is_local {
                    return format!("{}({})", sn, arg_strs.join(", "));
                }
                if arg_strs.is_empty() {
                    return format!("sv_call_fn({}, 0, NULL)", sn);
                }
                return format!("sv_call_fn({}, {}, (SVal[]){{{}}})", sn, arg_strs.len(), arg_strs.join(", "));
            }
            if arg_strs.is_empty() {
                format!("sv_call_fn({}, 0, NULL)", c_expr(callee, lambdas, lambda_id))
            } else {
                format!("sv_call_fn({}, {}, (SVal[]){{{}}})", c_expr(callee, lambdas, lambda_id), arg_strs.len(), arg_strs.join(", "))
            }
        }
        Expr::Index { target, index } => format!(
            "sv_get({}, {})",
            c_expr(target, lambdas, lambda_id),
            c_expr(index, lambdas, lambda_id)
        ),
        Expr::Member { target, field } => format!(
            "sv_map_get({}, \"{}\")",
            c_expr(target, lambdas, lambda_id),
            escape_str(field)
        ),
        Expr::FnExpr { params, body } => {
            *lambda_id += 1;
            let fn_name = format!("__steel_lambda_{}", *lambda_id);
            let mut decl = std::collections::HashSet::new();
            decl.insert("__st_this".to_string());
            let mut param_set = std::collections::HashSet::new();
            for (p, _) in params {
                param_set.insert(c_safe_ident(p));
            }
            let mut used = std::collections::HashSet::new();
            for st in body {
                collect_stmt_idents(st, &mut used);
            }
            let mut captures: Vec<String> = C_CUR_LOCALS.with(|l| {
                l.borrow()
                    .iter()
                    .filter(|v| *v != "__st_this" && !param_set.contains(*v) && used.contains(*v))
                    .cloned()
                    .collect()
            });
            captures.sort();
            let mut fn_code = format!(
                "static SVal {}(SVal __st_this, int _st_argc, SVal* _st_argv) {{\n    (void)__st_this;\n",
                fn_name
            );
            for cap in &captures {
                decl.insert(cap.clone());
                fn_code.push_str(&format!("    SVal {} = sv_map_get(__st_this, \"{}\");\n", cap, cap));
            }
            for (i, (p, def)) in params.iter().enumerate() {
                let sp = c_safe_ident(p);
                decl.insert(sp.clone());
                let fallback = def
                    .as_ref()
                    .map(|d| c_expr(d, lambdas, lambda_id))
                    .unwrap_or_else(|| "sv_null()".to_string());
                fn_code.push_str(&format!(
                    "    SVal {} = (_st_argc > {}) ? _st_argv[{}] : {};\n",
                    sp, i, i, fallback
                ));
            }
            let prev_locals = C_CUR_LOCALS.with(|l| l.borrow().clone());
            C_CUR_LOCALS.with(|l| *l.borrow_mut() = decl.clone());
            for st in body {
                fn_code.push_str(&c_stmt(st, 1, &mut decl, lambdas, lambda_id));
            }
            C_CUR_LOCALS.with(|l| *l.borrow_mut() = prev_locals);
            if !matches!(body.last(), Some(Stmt::Return(_))) {
                fn_code.push_str("    return sv_null();\n");
            }
            fn_code.push_str("}\n\n");
            lambdas.push_str(&fn_code);
            if captures.is_empty() {
                format!("sv_fn({})", fn_name)
            } else {
                let cap_keys: Vec<String> = captures.iter().map(|c| format!("\"{}\"", c)).collect();
                let cap_vals: Vec<String> = captures.clone();
                format!(
                    "sv_closure({}, {}, (const char*[]){{{}}}, (SVal[]){{{}}})",
                    fn_name,
                    captures.len(),
                    cap_keys.join(", "),
                    cap_vals.join(", ")
                )
            }
        }
        Expr::Range { start, end } => format!(
            "sv_range({}, {})",
            c_expr(start, lambdas, lambda_id),
            c_expr(end, lambdas, lambda_id)
        ),
        Expr::Spread(inner) => c_expr(inner, lambdas, lambda_id),
    }
}

// 6. GO TARGET (On-Demand Minimal Runtime + Unboxed Math & Range Loops)
fn emit_go(program: &Program) -> String {
    let mut go_top_vars = std::collections::HashSet::new();
    let mut go_top_fns = std::collections::HashSet::new();
    for stmt in &program.statements {
        match stmt {
            Stmt::FnDecl { name, .. } | Stmt::ClassDecl { name, .. } => { go_top_fns.insert(name.clone()); }
            Stmt::Let { name, .. } => { go_top_vars.insert(name.clone()); }
            Stmt::Assign { target: AssignTarget::Ident(name), .. } => { go_top_vars.insert(name.clone()); }
            _ => {}
        }
    }
    let mut go_pkg_globals = std::collections::BTreeSet::new();
    for stmt in &program.statements {
        if let Stmt::FnDecl { params, body, .. } = stmt {
            let mut locals: std::collections::HashSet<String> = params.iter().map(|(p, _)| p.clone()).collect();
            rs_collect_local_decls(body, &mut locals);
            let mut refs = std::collections::HashSet::new();
            rs_collect_ident_refs_stmts(body, &mut refs);
            for r in refs {
                if go_top_vars.contains(&r) && !locals.contains(&r) && !go_top_fns.contains(&r) {
                    go_pkg_globals.insert(r);
                }
            }
        }
    }
    let mut top_fns = String::new();
    for g in &go_pkg_globals {
        top_fns.push_str(&format!("var {} any\n", go_safe_ident(g)));
    }
    if !go_pkg_globals.is_empty() {
        top_fns.push('\n');
    }
    let mut main_body = String::new();
    for stmt in &program.statements {
        match stmt {
            Stmt::FnDecl { name, params, body } => {
                let p_sig: Vec<String> = params.iter().map(|(p, _)| format!("{} any", go_safe_ident(p))).collect();
                top_fns.push_str(&format!("func {}({}) any {{\n", go_safe_ident(name), p_sig.join(", ")));
                for st in body {
                    top_fns.push_str(&go_stmt(st, 1));
                }
                if !matches!(body.last(), Some(Stmt::Return(_))) {
                    top_fns.push_str("    return nil\n");
                }
                top_fns.push_str("}\n\n");
            }
            Stmt::ClassDecl {
                name,
                parent,
                methods,
            } => {
                if let Stmt::FnDecl { name, params, body } = desugar_class(name, parent, methods) {
                    let p_sig: Vec<String> =
                        params.iter().map(|(p, _)| format!("{} any", go_safe_ident(p))).collect();
                    top_fns.push_str(&format!("func {}({}) any {{\n", go_safe_ident(&name), p_sig.join(", ")));
                    for st in &body {
                        top_fns.push_str(&go_stmt(st, 1));
                    }
                    if !matches!(body.last(), Some(Stmt::Return(_))) {
                        top_fns.push_str("    return nil\n");
                    }
                    top_fns.push_str("}\n\n");
                }
            }
            Stmt::Let { name, value, .. } if go_pkg_globals.contains(name) => {
                main_body.push_str(&format!("    {} = {}\n", go_safe_ident(name), go_expr(value, 1)));
            }
            other => {
                main_body.push_str(&go_stmt(other, 1));
            }
        }
    }

    let user_code = format!("{}{}", top_fns, main_body);
    let (go_imports, rt) = go_runtime_helpers(&user_code);
    let mut out = String::from("package main\n\n");
    if !go_imports.is_empty() {
        out.push_str(&format!("import (\n    {}\n)\n\n", go_imports.join("\n    ")));
    }
    out.push_str(&rt);
    out.push('\n');
    out.push_str(&top_fns);
    out.push_str("func main() {\n");
    out.push_str(&main_body);
    out.push_str("}\n");
    out
}

fn go_runtime_helpers(user_code: &str) -> (Vec<&'static str>, String) {
    let mut rt = String::new();
    let needs_arr = user_code.contains("__SteelArr")
        || user_code.contains("__steel_range")
        || user_code.contains("__steel_push")
        || user_code.contains("__steel_pop")
        || user_code.contains("__steel_get")
        || user_code.contains("__steel_set")
        || user_code.contains("__steel_iter")
        || user_code.contains("__steel_split")
        || user_code.contains("__steel_reverse")
        || user_code.contains("__steel_keys")
        || user_code.contains("__steel_values")
        || user_code.contains("__steel_concat")
        || user_code.contains("__steel_unique")
        || user_code.contains("__steel_flat")
        || user_code.contains("__steel_zip")
        || user_code.contains("__steel_enumerate")
        || user_code.contains("__steel_json_")
        || user_code.contains("__steel_type")
        || user_code.contains("__steel_len")
        || user_code.contains("__steel_sum")
        || user_code.contains("__steel_avg")
        || user_code.contains("__steel_index_of")
        || user_code.contains("__steel_find")
        || user_code.contains("__steel_any")
        || user_code.contains("__steel_all")
        || user_code.contains("__steel_map")
        || user_code.contains("__steel_filter")
        || user_code.contains("__steel_reduce")
        || user_code.contains("__steel_sort")
        || user_code.contains("__steel_slice")
        || user_code.contains("__steel_contains")
        || user_code.contains("__steel_join");

    if user_code.contains("__steel_print(") {
        rt.push_str("func __steel_print(args ...any) { parts := make([]string, len(args)); for i, a := range args { if a == nil { parts[i] = \"null\" } else { parts[i] = fmt.Sprint(a) } }; fmt.Println(strings.Join(parts, \" \")) }\n");
    }
    if user_code.contains("__steel_read_file") {
        rt.push_str("func __steel_read_file(p any) any { b, err := os.ReadFile(fmt.Sprint(p)); if err != nil { return \"\" }; return string(b) }\n");
    }
    if user_code.contains("__steel_write_file") {
        rt.push_str("func __steel_write_file(p, c any) any { return os.WriteFile(fmt.Sprint(p), []byte(fmt.Sprint(c)), 0644) == nil }\n");
    }
    if user_code.contains("__steel_append_file") {
        rt.push_str("func __steel_append_file(p, c any) any { f, err := os.OpenFile(fmt.Sprint(p), os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644); if err != nil { return false }; defer f.Close(); _, err = f.WriteString(fmt.Sprint(c)); return err == nil }\n");
    }
    if user_code.contains("__steel_remove_file") {
        rt.push_str("func __steel_remove_file(p any) any { return os.Remove(fmt.Sprint(p)) == nil }\n");
    }
    if user_code.contains("__steel_file_exists") {
        rt.push_str("func __steel_file_exists(p any) any { _, err := os.Stat(fmt.Sprint(p)); return err == nil }\n");
    }
    if user_code.contains("__steel_env") {
        rt.push_str("func __steel_env(k any) any { v, ok := os.LookupEnv(fmt.Sprint(k)); if !ok { return nil }; return v }\n");
    }
    if user_code.contains("__steel_clock") {
        rt.push_str("func __steel_clock() float64 { return float64(time.Now().UnixNano()) / 1e9 }\n");
    }
    if user_code.contains("__steel_truthy") || user_code.contains("__steel_filter") || user_code.contains("__steel_find") || user_code.contains("__steel_any") || user_code.contains("__steel_all") || user_code.contains("__steel_assert") {
        rt.push_str(
            "func __steel_truthy(v any) bool {\n\
             \x20   if v == nil { return false }\n\
             \x20   if b, ok := v.(bool); ok { return b }\n\
             \x20   if n, ok := v.(float64); ok { return n != 0 }\n\
             \x20   if s, ok := v.(string); ok { return len(s) > 0 }\n\
             \x20   return true\n\
             }\n",
        );
    }
    if user_code.contains("__steel_assert") {
        rt.push_str("func __steel_assert(c any, m ...any) any { if !__steel_truthy(c) { msg := \"Assertion failed\"; if len(m) > 0 { msg = fmt.Sprint(m[0]) }; panic(msg) }; return true }\n");
    }
    if user_code.contains("__steel_num") || user_code.contains("__steel_add") || user_code.contains("__steel_range") || user_code.contains("__steel_get") || user_code.contains("__steel_set") || user_code.contains("__steel_slice") || user_code.contains("__steel_int") || user_code.contains("__steel_clamp") || user_code.contains("__steel_sign") || user_code.contains("__steel_repeat") || user_code.contains("__steel_pad_") || user_code.contains("__steel_from_char_code") || user_code.contains("__steel_sum") || user_code.contains("__steel_avg") {
        rt.push_str(
            "func __steel_num(v any) float64 {\n\
             \x20   if n, ok := v.(float64); ok { return n }\n\
             \x20   if n, ok := v.(int); ok { return float64(n) }\n\
             \x20   if b, ok := v.(bool); ok { if b { return 1 }; return 0 }\n\
             \x20   return 0\n\
             }\n",
        );
    }
    if user_code.contains("__steel_num_cast") || user_code.contains("__steel_int") {
        rt.push_str("func __steel_num_cast(v any) float64 { if s, ok := v.(string); ok { f, err := strconv.ParseFloat(strings.TrimSpace(s), 64); if err == nil { return f } }; return __steel_num(v) }\n");
    }
    if user_code.contains("__steel_int") {
        rt.push_str("func __steel_int(v any) float64 { return math.Trunc(__steel_num_cast(v)) }\n");
    }
    if user_code.contains("__steel_add") {
        rt.push_str(
            "func __steel_add(a, b any) any {\n\
             \x20   _, aStr := a.(string)\n\
             \x20   _, bStr := b.(string)\n\
             \x20   if aStr || bStr { return fmt.Sprintf(\"%v%v\", a, b) }\n\
             \x20   return __steel_num(a) + __steel_num(b)\n\
             }\n",
        );
    }
    if needs_arr {
        rt.push_str(
            "type __SteelArr struct { items []any }\n\
             func (a *__SteelArr) String() string { return fmt.Sprint(a.items) }\n\
             func __steel_push(arr any, v any) any { if a, ok := arr.(*__SteelArr); ok { a.items = append(a.items, v) }; return arr }\n\
             func __steel_pop(arr any) any { if a, ok := arr.(*__SteelArr); ok && len(a.items) > 0 { v := a.items[len(a.items)-1]; a.items = a.items[:len(a.items)-1]; return v }; return nil }\n\
             func __steel_get(arr any, idx any) any { if m, ok := arr.(map[string]any); ok { return m[fmt.Sprint(idx)] }; if a, ok := arr.(*__SteelArr); ok { i := int(__steel_num(idx)); if i < 0 { i += len(a.items) }; if i >= 0 && i < len(a.items) { return a.items[i] } }; return nil }\n\
             func __steel_set(arr any, idx any, v any) { if m, ok := arr.(map[string]any); ok { m[fmt.Sprint(idx)] = v; return }; if a, ok := arr.(*__SteelArr); ok { i := int(__steel_num(idx)); if i < 0 { i += len(a.items) }; if i >= 0 && i < len(a.items) { a.items[i] = v } } }\n\
             func __steel_iter(arr any) []any { if a, ok := arr.(*__SteelArr); ok { return a.items }; return nil }\n",
        );
    }
    if user_code.contains("__steel_type") {
        rt.push_str("func __steel_type(v any) string { if v == nil { return \"null\" }; switch v.(type) { case float64, int: return \"number\"; case string: return \"string\"; case bool: return \"boolean\"; case *__SteelArr: return \"array\"; case map[string]any: return \"map\"; default: return \"function\" } }\n");
    }
    if user_code.contains("__steel_has_key") {
        rt.push_str("func __steel_has_key(x, k any) bool { if m, ok := x.(map[string]any); ok { _, ex := m[fmt.Sprint(k)]; return ex }; return false }\n");
    }
    if user_code.contains("__steel_delete_key") {
        rt.push_str("func __steel_delete_key(x, k any) bool { if m, ok := x.(map[string]any); ok { ks := fmt.Sprint(k); if _, ex := m[ks]; ex { delete(m, ks); return true } }; return false }\n");
    }
    if user_code.contains("__steel_concat") {
        rt.push_str("func __steel_concat(a, b any) *__SteelArr { ai, bi := __steel_iter(a), __steel_iter(b); out := make([]any, 0, len(ai)+len(bi)); out = append(out, ai...); out = append(out, bi...); return &__SteelArr{items: out} }\n");
    }
    if user_code.contains("__steel_contains") {
        rt.push_str("func __steel_contains(x, v any) any { if m, ok := x.(map[string]any); ok { _, ex := m[fmt.Sprint(v)]; return ex }; if a, ok := x.(*__SteelArr); ok { for _, it := range a.items { if fmt.Sprint(it) == fmt.Sprint(v) { return true } }; return false }; return strings.Contains(fmt.Sprint(x), fmt.Sprint(v)) }\n");
    }
    if user_code.contains("__steel_index_of") {
        rt.push_str("func __steel_index_of(x, v any) float64 { if a, ok := x.(*__SteelArr); ok { for i, it := range a.items { if fmt.Sprint(it) == fmt.Sprint(v) { return float64(i) } }; return -1 }; s := fmt.Sprint(x); sub := fmt.Sprint(v); idx := strings.Index(s, sub); if idx < 0 { return -1 }; return float64(len([]rune(s[:idx]))) }\n");
    }
    if user_code.contains("__steel_repeat") {
        rt.push_str("func __steel_repeat(s, n any) string { c := int(__steel_num(n)); if c <= 0 { return \"\" }; return strings.Repeat(fmt.Sprint(s), c) }\n");
    }
    if user_code.contains("__steel_pad_start") {
        rt.push_str("func __steel_pad_start(s, w, ch any) string { ss := fmt.Sprint(s); cs := fmt.Sprint(ch); rs := []rune(ss); ww := int(__steel_num(w)); if len(rs) >= ww || len(cs) == 0 { return ss }; pr := []rune(cs); need := ww - len(rs); out := make([]rune, 0, ww); for i := 0; i < need; i++ { out = append(out, pr[i%len(pr)]) }; out = append(out, rs...); return string(out) }\n");
    }
    if user_code.contains("__steel_pad_end") {
        rt.push_str("func __steel_pad_end(s, w, ch any) string { ss := fmt.Sprint(s); cs := fmt.Sprint(ch); rs := []rune(ss); ww := int(__steel_num(w)); if len(rs) >= ww || len(cs) == 0 { return ss }; pr := []rune(cs); need := ww - len(rs); out := make([]rune, 0, ww); out = append(out, rs...); for i := 0; i < need; i++ { out = append(out, pr[i%len(pr)]) }; return string(out) }\n");
    }
    if user_code.contains("__steel_char_code") {
        rt.push_str("func __steel_char_code(s any) float64 { rs := []rune(fmt.Sprint(s)); if len(rs) == 0 { return 0 }; return float64(rs[0]) }\n");
    }
    if user_code.contains("__steel_from_char_code") {
        rt.push_str("func __steel_from_char_code(n any) string { c := int(__steel_num(n)); if c < 0 { c = 0 }; return string(rune(c)) }\n");
    }
    if user_code.contains("__steel_split") {
        rt.push_str("func __steel_split(s, sep any) *__SteelArr { ps := strings.Split(fmt.Sprint(s), fmt.Sprint(sep)); out := make([]any, len(ps)); for i, p := range ps { out[i] = p }; return &__SteelArr{items: out} }\n");
    }
    if user_code.contains("__steel_join") {
        rt.push_str("func __steel_join(arr, sep any) any { its := __steel_iter(arr); ps := make([]string, len(its)); for i, v := range its { ps[i] = fmt.Sprint(v) }; return strings.Join(ps, fmt.Sprint(sep)) }\n");
    }
    if user_code.contains("__steel_reverse") {
        rt.push_str("func __steel_reverse(x any) any { if a, ok := x.(*__SteelArr); ok { n := len(a.items); out := make([]any, n); for i := 0; i < n; i++ { out[i] = a.items[n-1-i] }; return &__SteelArr{items: out} }; rs := []rune(fmt.Sprint(x)); for i, j := 0, len(rs)-1; i < j; i, j = i+1, j-1 { rs[i], rs[j] = rs[j], rs[i] }; return string(rs) }\n");
    }
    if user_code.contains("__steel_keys") {
        rt.push_str("func __steel_keys(x any) *__SteelArr { out := []any{}; if m, ok := x.(map[string]any); ok { for k := range m { out = append(out, k) } }; return &__SteelArr{items: out} }\n");
    }
    if user_code.contains("__steel_values") {
        rt.push_str("func __steel_values(x any) *__SteelArr { out := []any{}; if m, ok := x.(map[string]any); ok { for _, v := range m { out = append(out, v) } }; return &__SteelArr{items: out} }\n");
    }
    if user_code.contains("__steel_slice") {
        rt.push_str("func __steel_slice(x, s, e any) any { si := int(__steel_num(s)); if a, ok := x.(*__SteelArr); ok { ei := len(a.items); if e != nil { ei = int(__steel_num(e)) }; if si < 0 { si = 0 }; if ei > len(a.items) { ei = len(a.items) }; if si > ei { si = ei }; cp := make([]any, ei-si); copy(cp, a.items[si:ei]); return &__SteelArr{items: cp} }; rs := []rune(fmt.Sprint(x)); ei := len(rs); if e != nil { ei = int(__steel_num(e)) }; if si < 0 { si = 0 }; if ei > len(rs) { ei = len(rs) }; if si > ei { si = ei }; return string(rs[si:ei]) }\n");
    }
    if user_code.contains("__steel_sort") {
        rt.push_str("func __steel_sort(x any) *__SteelArr { out := []any{}; if a, ok := x.(*__SteelArr); ok { out = append(out, a.items...); sort.Slice(out, func(i, j int) bool { return fmt.Sprint(out[i]) < fmt.Sprint(out[j]) }) }; return &__SteelArr{items: out} }\n");
    }
    if user_code.contains("__steel_sum") {
        rt.push_str("func __steel_sum(x any) float64 { s := 0.0; for _, v := range __steel_iter(x) { if n, ok := v.(float64); ok { s += n } }; return s }\n");
    }
    if user_code.contains("__steel_avg") {
        rt.push_str("func __steel_avg(x any) float64 { its := __steel_iter(x); if len(its) == 0 { return 0 }; s := 0.0; for _, v := range its { if n, ok := v.(float64); ok { s += n } }; return s / float64(len(its)) }\n");
    }
    if user_code.contains("__steel_unique") {
        rt.push_str("func __steel_unique(x any) *__SteelArr { out := []any{}; for _, v := range __steel_iter(x) { dup := false; for _, u := range out { if fmt.Sprint(u) == fmt.Sprint(v) { dup = true; break } }; if !dup { out = append(out, v) } }; return &__SteelArr{items: out} }\n");
    }
    if user_code.contains("__steel_flat") {
        rt.push_str("func __steel_flat(x any) *__SteelArr { out := []any{}; for _, v := range __steel_iter(x) { if a, ok := v.(*__SteelArr); ok { out = append(out, a.items...) } else { out = append(out, v) } }; return &__SteelArr{items: out} }\n");
    }
    if user_code.contains("__steel_zip") {
        rt.push_str("func __steel_zip(a, b any) *__SteelArr { ai, bi := __steel_iter(a), __steel_iter(b); n := len(ai); if len(bi) < n { n = len(bi) }; out := make([]any, n); for i := 0; i < n; i++ { out[i] = &__SteelArr{items: []any{ai[i], bi[i]}} }; return &__SteelArr{items: out} }\n");
    }
    if user_code.contains("__steel_enumerate") {
        rt.push_str("func __steel_enumerate(a any) *__SteelArr { ai := __steel_iter(a); out := make([]any, len(ai)); for i, v := range ai { out[i] = &__SteelArr{items: []any{float64(i), v}} }; return &__SteelArr{items: out} }\n");
    }
    if user_code.contains("__steel_clamp") {
        rt.push_str("func __steel_clamp(x, lo, hi float64) float64 { if x < lo { return lo }; if x > hi { return hi }; return x }\n");
    }
    if user_code.contains("__steel_sign") {
        rt.push_str("func __steel_sign(x float64) float64 { if x > 0 { return 1 }; if x < 0 { return -1 }; return 0 }\n");
    }
    if user_code.contains("__steel_json_stringify") {
        rt.push_str("func __steel_to_plain(v any) any { if a, ok := v.(*__SteelArr); ok { out := make([]any, len(a.items)); for i, it := range a.items { out[i] = __steel_to_plain(it) }; return out }; if m, ok := v.(map[string]any); ok { out := make(map[string]any, len(m)); for k, val := range m { out[k] = __steel_to_plain(val) }; return out }; return v }\nfunc __steel_json_stringify(v any) string { b, err := json.Marshal(__steel_to_plain(v)); if err != nil { return \"null\" }; return string(b) }\n");
    }
    if user_code.contains("__steel_json_parse") {
        rt.push_str("func __steel_from_plain(v any) any { if a, ok := v.([]any); ok { out := make([]any, len(a)); for i, it := range a { out[i] = __steel_from_plain(it) }; return &__SteelArr{items: out} }; if m, ok := v.(map[string]any); ok { for k, val := range m { m[k] = __steel_from_plain(val) }; return m }; return v }\nfunc __steel_json_parse(s any) any { var out any; _ = json.Unmarshal([]byte(fmt.Sprint(s)), &out); return __steel_from_plain(out) }\n");
    }
    if user_code.contains("__steel_hash") {
        rt.push_str("func __steel_hash(v any) string { var h uint64 = 0xcbf29ce484222325; for _, b := range []byte(fmt.Sprint(v)) { h ^= uint64(b); h *= 0x0100000001b3 }; return fmt.Sprintf(\"%016x\", h) }\n");
    }
    if user_code.contains("__steel_call") || user_code.contains("__steel_map") || user_code.contains("__steel_filter") || user_code.contains("__steel_reduce") || user_code.contains("__steel_find") || user_code.contains("__steel_any") || user_code.contains("__steel_all") {
        rt.push_str(
            "func __steel_call(fn any, args ...any) any {\n\
             \x20   a := func(i int) any { if i < len(args) { return args[i] }; return nil }\n\
             \x20   switch f := fn.(type) {\n\
             \x20   case func(...any) any: return f(args...)\n\
             \x20   case func() any: return f()\n\
             \x20   case func(any) any: return f(a(0))\n\
             \x20   case func(any, any) any: return f(a(0), a(1))\n\
             \x20   case func(any, any, any) any: return f(a(0), a(1), a(2))\n\
             \x20   case func(any, any, any, any) any: return f(a(0), a(1), a(2), a(3))\n\
             \x20   case func(any, any, any, any, any) any: return f(a(0), a(1), a(2), a(3), a(4))\n\
             \x20   case func(any, any, any, any, any, any) any: return f(a(0), a(1), a(2), a(3), a(4), a(5))\n\
             \x20   case func(any, any, any, any, any, any, any) any: return f(a(0), a(1), a(2), a(3), a(4), a(5), a(6))\n\
             \x20   case func(any, any, any, any, any, any, any, any) any: return f(a(0), a(1), a(2), a(3), a(4), a(5), a(6), a(7))\n\
             \x20   }\n\
             \x20   return nil\n\
             }\n",
        );
    }
    if user_code.contains("__steel_len") {
        rt.push_str(
            "func __steel_len(v any) float64 {\n\
             \x20   if a, ok := v.(*__SteelArr); ok { return float64(len(a.items)) }\n\
             \x20   if s, ok := v.(string); ok { return float64(len([]rune(s))) }\n\
             \x20   if m, ok := v.(map[string]any); ok { return float64(len(m)) }\n\
             \x20   return 0\n\
             }\n",
        );
    }
    if user_code.contains("__steel_map") || user_code.contains("__steel_filter") || user_code.contains("__steel_reduce") || user_code.contains("__steel_find") || user_code.contains("__steel_any") || user_code.contains("__steel_all") {
        rt.push_str(
            "func __steel_map(arr any, fn any) *__SteelArr { out := &__SteelArr{items: []any{}}; for _, v := range __steel_iter(arr) { out.items = append(out.items, __steel_call(fn, v)) }; return out }\n\
             func __steel_filter(arr any, fn any) *__SteelArr { out := &__SteelArr{items: []any{}}; for _, v := range __steel_iter(arr) { if __steel_truthy(__steel_call(fn, v)) { out.items = append(out.items, v) } }; return out }\n\
             func __steel_reduce(arr any, fn any, init any) any { its := __steel_iter(arr); start := 0; acc := init; if acc == nil && len(its) > 0 { acc = its[0]; start = 1 }; for i := start; i < len(its); i++ { acc = __steel_call(fn, acc, its[i]) }; return acc }\n\
             func __steel_find(arr any, fn any) any { for _, v := range __steel_iter(arr) { if __steel_truthy(__steel_call(fn, v)) { return v } }; return nil }\n\
             func __steel_any(arr any, fn any) bool { for _, v := range __steel_iter(arr) { if __steel_truthy(__steel_call(fn, v)) { return true } }; return false }\n\
             func __steel_all(arr any, fn any) bool { for _, v := range __steel_iter(arr) { if !__steel_truthy(__steel_call(fn, v)) { return false } }; return true }\n",
        );
    }
    if user_code.contains("__steel_range") {
        rt.push_str(
            "func __steel_range(a, b any, stepArg ...any) *__SteelArr {\n\
             \x20   s, e, step := int(__steel_num(a)), int(__steel_num(b)), 1\n\
             \x20   if len(stepArg) > 0 && stepArg[0] != nil { step = int(__steel_num(stepArg[0])) }\n\
             \x20   out := []any{}\n\
             \x20   if step > 0 { for i := s; i < e; i += step { out = append(out, float64(i)) } } else if step < 0 { for i := s; i > e; i += step { out = append(out, float64(i)) } }\n\
             \x20   return &__SteelArr{items: out}\n\
             }\n",
        );
    }

    let combined = format!("{}\n{}", user_code, rt);
    let mut imports = Vec::new();
    if combined.contains("fmt.") { imports.push("\"fmt\""); }
    if combined.contains("math.") { imports.push("\"math\""); }
    if combined.contains("os.") { imports.push("\"os\""); }
    if combined.contains("strings.") { imports.push("\"strings\""); }
    if combined.contains("sort.") { imports.push("\"sort\""); }
    if combined.contains("strconv.") { imports.push("\"strconv\""); }
    if combined.contains("json.") { imports.push("\"encoding/json\""); }
    if combined.contains("time.") { imports.push("\"time\""); }
    (imports, rt)
}

fn go_num_expr(expr: &Expr, lvl: usize) -> String {
    match expr {
        Expr::Number(n) => format!("float64({})", fmt_num(*n)),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr: inner,
        } => format!("(-{})", go_num_expr(inner, lvl)),
        Expr::Binary { left, op, right } => match op {
            BinaryOp::Add if !self_might_be_string(left) && !self_might_be_string(right) => {
                format!("({} + {})", go_num_expr(left, lvl), go_num_expr(right, lvl))
            }
            BinaryOp::Sub => format!("({} - {})", go_num_expr(left, lvl), go_num_expr(right, lvl)),
            BinaryOp::Mul => format!("({} * {})", go_num_expr(left, lvl), go_num_expr(right, lvl)),
            BinaryOp::Div => format!("({} / {})", go_num_expr(left, lvl), go_num_expr(right, lvl)),
            BinaryOp::Mod => format!(
                "float64(int({}) % int({}))",
                go_num_expr(left, lvl),
                go_num_expr(right, lvl)
            ),
            BinaryOp::Pow => format!(
                "math.Pow({}, {})",
                go_num_expr(left, lvl),
                go_num_expr(right, lvl)
            ),
            _ => format!("__steel_num({})", go_expr(expr, lvl)),
        },
        _ => format!("__steel_num({})", go_expr(expr, lvl)),
    }
}

fn go_cond_expr(expr: &Expr, lvl: usize) -> String {
    match expr {
        Expr::Binary { left, op, right } => match op {
            BinaryOp::Lt => format!("({} < {})", go_num_expr(left, lvl), go_num_expr(right, lvl)),
            BinaryOp::Gt => format!("({} > {})", go_num_expr(left, lvl), go_num_expr(right, lvl)),
            BinaryOp::Lte => format!("({} <= {})", go_num_expr(left, lvl), go_num_expr(right, lvl)),
            BinaryOp::Gte => format!("({} >= {})", go_num_expr(left, lvl), go_num_expr(right, lvl)),
            BinaryOp::Eq => format!("(any({}) == any({}))", go_expr(left, lvl), go_expr(right, lvl)),
            BinaryOp::Neq => format!("(any({}) != any({}))", go_expr(left, lvl), go_expr(right, lvl)),
            BinaryOp::And => format!(
                "({} && {})",
                go_cond_expr(left, lvl),
                go_cond_expr(right, lvl)
            ),
            BinaryOp::Or => format!(
                "({} || {})",
                go_cond_expr(left, lvl),
                go_cond_expr(right, lvl)
            ),
            _ => format!("__steel_truthy({})", go_expr(expr, lvl)),
        },
        Expr::Unary {
            op: UnaryOp::Not,
            expr: inner,
        } => format!("(!{})", go_cond_expr(inner, lvl)),
        Expr::Bool(b) => b.to_string(),
        _ => format!("__steel_truthy({})", go_expr(expr, lvl)),
    }
}

fn go_safe_ident(name: &str) -> String {
    match name {
        "break" | "case" | "chan" | "const" | "continue" | "default" | "defer" | "else"
        | "fallthrough" | "for" | "func" | "go" | "goto" | "if" | "import" | "interface"
        | "map" | "package" | "range" | "return" | "select" | "struct" | "switch" | "type"
        | "var" | "string" | "int" | "float64" | "bool" | "byte" | "rune" | "error" | "any" => {
            format!("_st_{}", name)
        }
        _ => name.to_string(),
    }
}

fn go_try_stmt(stmt: &Stmt, lvl: usize) -> String {
    let pad = indent(lvl);
    match stmt {
        Stmt::Return(val) => {
            let v = val.as_ref().map(|e| go_expr(e, lvl)).unwrap_or_else(|| "nil".to_string());
            format!("{}__try_ret = {}\n{}__try_has_ret = true\n{}return\n", pad, v, pad, pad)
        }
        Stmt::If { cond, then_branch, else_branch } => {
            let mut s = format!("{}if __steel_truthy({}) {{\n", pad, go_expr(cond, lvl));
            for st in then_branch { s.push_str(&go_try_stmt(st, lvl + 1)); }
            if let Some(eb) = else_branch {
                s.push_str(&format!("{}}} else {{\n", pad));
                for st in eb { s.push_str(&go_try_stmt(st, lvl + 1)); }
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::While { cond, body } => {
            let mut s = format!("{}for __steel_truthy({}) {{\n", pad, go_expr(cond, lvl));
            for st in body { s.push_str(&go_try_stmt(st, lvl + 1)); }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        other => go_stmt(other, lvl),
    }
}

fn go_stmt(stmt: &Stmt, lvl: usize) -> String {
    let pad = indent(lvl);
    match stmt {
        Stmt::Import(p) => format!("{}// import \"{}\"\n", pad, escape_str(p)),
        Stmt::ClassDecl {
            name,
            parent,
            methods,
        } => go_stmt(&desugar_class(name, parent, methods), lvl),
        Stmt::TryCatch {
            try_body,
            err_var,
            catch_body,
        } => {
            let mut try_code = String::new();
            for st in try_body {
                try_code.push_str(&go_try_stmt(st, lvl + 1));
            }
            let mut catch_code = String::new();
            for st in catch_body {
                catch_code.push_str(&go_stmt(st, lvl + 1));
            }
            let has_ret = try_code.contains("__try_has_ret");
            let mut s = String::new();
            s.push_str(&format!(
                "{}{{\n{}    var __has_err bool\n{}    var __caught any\n",
                pad, pad, pad
            ));
            if has_ret {
                s.push_str(&format!("{}    var __try_ret any\n{}    var __try_has_ret bool\n", pad, pad));
            }
            s.push_str(&format!(
                "{}    func() {{\n{}        defer func() {{\n{}            if __r := recover(); __r != nil {{\n{}                __has_err = true\n{}                __caught = fmt.Sprint(__r)\n{}            }}\n{}        }}()\n",
                pad, pad, pad, pad, pad, pad, pad
            ));
            s.push_str(&try_code);
            s.push_str(&format!("{}    }}()\n", pad));
            if has_ret {
                s.push_str(&format!("{}    if __try_has_ret {{ return __try_ret }}\n", pad));
            }
            s.push_str(&format!(
                "{}    if __has_err {{\n{}        var {} any = __caught\n{}        _ = {}\n",
                pad, pad, go_safe_ident(err_var), pad, go_safe_ident(err_var)
            ));
            s.push_str(&catch_code);
            s.push_str(&format!("{}    }}\n{}}}\n", pad, pad));
            s
        }
        Stmt::Throw(e) => format!("{}panic({})\n", pad, go_expr(e, lvl)),
        Stmt::Test { name, body } => {
            let mut s = format!("{}// test \"{}\"\n{}{{\n", pad, escape_str(name), pad);
            for st in body {
                s.push_str(&go_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::Defer(inner) => format!("{}defer func() {{\n{}{}}}()\n", pad, go_stmt(inner, lvl + 1), pad),
        Stmt::Let { name, value, .. } => {
            let sn = go_safe_ident(name);
            format!(
                "{}var {} any = {}\n{}_ = {}\n",
                pad,
                sn,
                go_expr(value, lvl),
                pad,
                sn
            )
        }
        Stmt::Assign { target, value } => match target {
            AssignTarget::Ident(n) => format!("{}{} = {}\n", pad, go_safe_ident(n), go_expr(value, lvl)),
            AssignTarget::Index { target, index } => format!(
                "{}__steel_set({}, {}, {})\n",
                pad,
                go_expr(target, lvl),
                go_expr(index, lvl),
                go_expr(value, lvl)
            ),
            AssignTarget::Member { target, field } => format!(
                "{}{}.(map[string]any)[\"{}\"] = {}\n",
                pad,
                go_expr(target, lvl),
                escape_str(field),
                go_expr(value, lvl)
            ),
        },
        Stmt::FnDecl { name, params, body } => {
            let p_sig: Vec<String> = params.iter().map(|(p, _)| format!("{} any", p)).collect();
            let mut s = format!(
                "{}var {} = func({}) any {{\n",
                pad,
                name,
                p_sig.join(", ")
            );
            for st in body {
                s.push_str(&go_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}    return nil\n{}}}\n", pad, pad));
            s
        }
        Stmt::Return(expr) => match expr {
            Some(e) => format!("{}return {}\n", pad, go_expr(e, lvl)),
            None => format!("{}return nil\n", pad),
        },
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let mut s = format!("{}if {} {{\n", pad, go_cond_expr(cond, lvl));
            for st in then_branch {
                s.push_str(&go_stmt(st, lvl + 1));
            }
            if let Some(eb) = else_branch {
                s.push_str(&format!("{}}} else {{\n", pad));
                for st in eb {
                    s.push_str(&go_stmt(st, lvl + 1));
                }
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::While { cond, body } => {
            let mut s = format!("{}for {} {{\n", pad, go_cond_expr(cond, lvl));
            for st in body {
                s.push_str(&go_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::For {
            var,
            iterable,
            body,
        } => {
            if let Expr::Range { start, end } = iterable {
                let idx_var = format!("__i_{}", var);
                let mut s = format!(
                    "{}for {} := int({}); {} < int({}); {}++ {{\n{}    var {} any = float64({})\n{}    _ = {}\n",
                    pad,
                    idx_var,
                    go_num_expr(start, lvl),
                    idx_var,
                    go_num_expr(end, lvl),
                    idx_var,
                    pad,
                    var,
                    idx_var,
                    pad,
                    var
                );
                for st in body {
                    s.push_str(&go_stmt(st, lvl + 1));
                }
                s.push_str(&format!("{}}}\n", pad));
                return s;
            }
            let mut s = format!(
                "{}for _, {} := range __steel_iter({}) {{\n{}    _ = {}\n",
                pad,
                var,
                go_expr(iterable, lvl),
                pad,
                var
            );
            for st in body {
                s.push_str(&go_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::Break => format!("{}break\n", pad),
        Stmt::Continue => format!("{}continue\n", pad),
        Stmt::Expr(e) => format!("{}{}\n", pad, go_expr(e, lvl)),
    }
}

fn go_expr(expr: &Expr, lvl: usize) -> String {
    match expr {
        Expr::Number(n) => format!("float64({})", fmt_num(*n)),
        Expr::String(s) => format!("\"{}\"", escape_str(s)),
        Expr::Bool(b) => b.to_string(),
        Expr::Null => "nil".to_string(),
        Expr::Ident(name) => match name.as_str() {
            "print" | "println" => "fmt.Println".to_string(),
            "str" => "fmt.Sprint".to_string(),
            "range" => "__steel_range".to_string(),
            "PI" => "math.Pi".to_string(),
            "E" => "math.E".to_string(),
            _ => go_safe_ident(name),
        },
        Expr::Array(items) => {
            let inner: Vec<String> = items.iter().map(|e| go_expr(e, lvl)).collect();
            format!("&__SteelArr{{items: []any{{{}}}}}", inner.join(", "))
        }
        Expr::Map(entries) => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("\"{}\": {}", escape_str(k), go_expr(v, lvl)))
                .collect();
            format!("map[string]any{{{}}}", inner.join(", "))
        }
        Expr::New { class_name, args } => {
            let arg_strs: Vec<String> = args.iter().map(|a| go_expr(a, lvl)).collect();
            format!("{}({})", class_name, arg_strs.join(", "))
        }
        Expr::Ternary {
            cond,
            then_expr,
            else_expr,
        } => format!(
            "func() any {{ if {} {{ return {} }}; return {} }}()",
            go_cond_expr(cond, lvl),
            go_expr(then_expr, lvl),
            go_expr(else_expr, lvl)
        ),
        Expr::Binary { left, op, right } => match op {
            BinaryOp::Add => format!("__steel_add({}, {})", go_expr(left, lvl), go_expr(right, lvl)),
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod | BinaryOp::Pow => {
                go_num_expr(expr, lvl)
            }
            BinaryOp::BitAnd => format!(
                "float64(int({}) & int({}))",
                go_num_expr(left, lvl),
                go_num_expr(right, lvl)
            ),
            BinaryOp::BitOr => format!(
                "float64(int({}) | int({}))",
                go_num_expr(left, lvl),
                go_num_expr(right, lvl)
            ),
            BinaryOp::BitXor => format!(
                "float64(int({}) ^ int({}))",
                go_num_expr(left, lvl),
                go_num_expr(right, lvl)
            ),
            BinaryOp::Shl => format!(
                "float64(int({}) << uint({}))",
                go_num_expr(left, lvl),
                go_num_expr(right, lvl)
            ),
            BinaryOp::Shr => format!(
                "float64(int({}) >> uint({}))",
                go_num_expr(left, lvl),
                go_num_expr(right, lvl)
            ),
            BinaryOp::NullCoalesce => {
                let l = go_expr(left, lvl);
                let r = go_expr(right, lvl);
                format!(
                    "func() any {{ if {} != nil {{ return {} }}; return {} }}()",
                    l, l, r
                )
            }
            BinaryOp::Eq
            | BinaryOp::Neq
            | BinaryOp::Lt
            | BinaryOp::Gt
            | BinaryOp::Lte
            | BinaryOp::Gte
            | BinaryOp::And
            | BinaryOp::Or => go_cond_expr(expr, lvl),
        },
        Expr::Unary { op, expr: inner } => match op {
            UnaryOp::Neg => go_num_expr(expr, lvl),
            UnaryOp::Not => go_cond_expr(expr, lvl),
            UnaryOp::BitNot => format!("float64(^int({}))", go_num_expr(inner, lvl)),
        },
        Expr::Call { callee, args } => {
            if let Expr::Ident(name) = &**callee {
                match name.as_str() {
                    "len" if args.len() == 1 => {
                        return format!("__steel_len({})", go_expr(&args[0], lvl));
                    }
                    "print" | "println" => {
                        let arg_strs: Vec<String> = args.iter().map(|a| go_expr(a, lvl)).collect();
                        return format!("__steel_print({})", arg_strs.join(", "));
                    }
                    "str" if args.len() == 1 => {
                        return format!("fmt.Sprint({})", go_expr(&args[0], lvl));
                    }
                    "num" if args.len() == 1 => {
                        return format!("__steel_num_cast({})", go_expr(&args[0], lvl));
                    }
                    "int" if args.len() == 1 => {
                        return format!("__steel_int({})", go_expr(&args[0], lvl));
                    }
                    "bool" if args.len() == 1 => {
                        return format!("__steel_truthy({})", go_expr(&args[0], lvl));
                    }
                    "type" if args.len() == 1 => {
                        return format!("__steel_type({})", go_expr(&args[0], lvl));
                    }
                    "assert" if !args.is_empty() => {
                        let a: Vec<String> = args.iter().map(|x| go_expr(x, lvl)).collect();
                        return format!("__steel_assert({})", a.join(", "));
                    }
                    "push" if args.len() == 2 => {
                        return format!("__steel_push({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "pop" if args.len() == 1 => {
                        return format!("__steel_pop({})", go_expr(&args[0], lvl));
                    }
                    "has_key" if args.len() == 2 => {
                        return format!("__steel_has_key({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "delete_key" if args.len() == 2 => {
                        return format!("__steel_delete_key({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "concat" if args.len() == 2 => {
                        return format!("__steel_concat({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "index_of" if args.len() == 2 => {
                        return format!("__steel_index_of({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "repeat" if args.len() == 2 => {
                        return format!("__steel_repeat({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "pad_start" if args.len() >= 2 => {
                        let ch = args.get(2).map(|a| go_expr(a, lvl)).unwrap_or_else(|| "\" \"".to_string());
                        return format!("__steel_pad_start({}, {}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl), ch);
                    }
                    "pad_end" if args.len() >= 2 => {
                        let ch = args.get(2).map(|a| go_expr(a, lvl)).unwrap_or_else(|| "\" \"".to_string());
                        return format!("__steel_pad_end({}, {}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl), ch);
                    }
                    "char_code" if args.len() == 1 => {
                        return format!("__steel_char_code({})", go_expr(&args[0], lvl));
                    }
                    "from_char_code" if args.len() == 1 => {
                        return format!("__steel_from_char_code({})", go_expr(&args[0], lvl));
                    }
                    "range" if args.len() >= 2 => {
                        let a: Vec<String> = args.iter().map(|x| go_expr(x, lvl)).collect();
                        return format!("__steel_range({})", a.join(", "));
                    }
                    "map" if args.len() == 2 => {
                        return format!("__steel_map({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "filter" if args.len() == 2 => {
                        return format!("__steel_filter({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "reduce" if args.len() >= 2 => {
                        let init = args.get(2).map(|a| go_expr(a, lvl)).unwrap_or_else(|| "nil".to_string());
                        return format!("__steel_reduce({}, {}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl), init);
                    }
                    "find" if args.len() == 2 => {
                        return format!("__steel_find({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "any" if args.len() == 2 => {
                        return format!("__steel_any({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "all" if args.len() == 2 => {
                        return format!("__steel_all({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "sum" if args.len() == 1 => {
                        return format!("__steel_sum({})", go_expr(&args[0], lvl));
                    }
                    "avg" if args.len() == 1 => {
                        return format!("__steel_avg({})", go_expr(&args[0], lvl));
                    }
                    "unique" if args.len() == 1 => {
                        return format!("__steel_unique({})", go_expr(&args[0], lvl));
                    }
                    "flat" if args.len() == 1 => {
                        return format!("__steel_flat({})", go_expr(&args[0], lvl));
                    }
                    "zip" if args.len() == 2 => {
                        return format!("__steel_zip({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "enumerate" if args.len() == 1 => {
                        return format!("__steel_enumerate({})", go_expr(&args[0], lvl));
                    }
                    "clamp" if args.len() == 3 => {
                        return format!("__steel_clamp({}, {}, {})", go_num_expr(&args[0], lvl), go_num_expr(&args[1], lvl), go_num_expr(&args[2], lvl));
                    }
                    "sign" if args.len() == 1 => {
                        return format!("__steel_sign({})", go_num_expr(&args[0], lvl));
                    }
                    "atan2" if args.len() == 2 => {
                        return format!("math.Atan2({}, {})", go_num_expr(&args[0], lvl), go_num_expr(&args[1], lvl));
                    }
                    "log" if args.len() == 1 => {
                        return format!("math.Log({})", go_num_expr(&args[0], lvl));
                    }
                    "exp" if args.len() == 1 => {
                        return format!("math.Exp({})", go_num_expr(&args[0], lvl));
                    }
                    "json_stringify" if args.len() == 1 => {
                        return format!("__steel_json_stringify({})", go_expr(&args[0], lvl));
                    }
                    "json_parse" if args.len() == 1 => {
                        return format!("__steel_json_parse({})", go_expr(&args[0], lvl));
                    }
                    "hash" if args.len() == 1 => {
                        return format!("__steel_hash({})", go_expr(&args[0], lvl));
                    }
                    "append_file" if args.len() == 2 => {
                        return format!("__steel_append_file({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "remove_file" if args.len() == 1 => {
                        return format!("__steel_remove_file({})", go_expr(&args[0], lvl));
                    }
                    "clock" if args.is_empty() => {
                        return "__steel_clock()".to_string();
                    }
                    "floor" if args.len() == 1 => {
                        return format!("math.Floor({})", go_num_expr(&args[0], lvl));
                    }
                    "ceil" if args.len() == 1 => {
                        return format!("math.Ceil({})", go_num_expr(&args[0], lvl));
                    }
                    "sqrt" if args.len() == 1 => {
                        return format!("math.Sqrt({})", go_num_expr(&args[0], lvl));
                    }
                    "sin" if args.len() == 1 => {
                        return format!("math.Sin({})", go_num_expr(&args[0], lvl));
                    }
                    "cos" if args.len() == 1 => {
                        return format!("math.Cos({})", go_num_expr(&args[0], lvl));
                    }
                    "tan" if args.len() == 1 => {
                        return format!("math.Tan({})", go_num_expr(&args[0], lvl));
                    }
                    "round" if args.len() == 1 => {
                        return format!("math.Round({})", go_num_expr(&args[0], lvl));
                    }
                    "pow" if args.len() == 2 => {
                        return format!("math.Pow({}, {})", go_num_expr(&args[0], lvl), go_num_expr(&args[1], lvl));
                    }
                    "min" if args.len() == 2 => {
                        return format!("math.Min({}, {})", go_num_expr(&args[0], lvl), go_num_expr(&args[1], lvl));
                    }
                    "max" if args.len() == 2 => {
                        return format!("math.Max({}, {})", go_num_expr(&args[0], lvl), go_num_expr(&args[1], lvl));
                    }
                    "abs" if args.len() == 1 => {
                        return format!("math.Abs({})", go_num_expr(&args[0], lvl));
                    }
                    "read_file" if args.len() == 1 => {
                        return format!("__steel_read_file({})", go_expr(&args[0], lvl));
                    }
                    "write_file" if args.len() == 2 => {
                        return format!("__steel_write_file({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "file_exists" if args.len() == 1 => {
                        return format!("__steel_file_exists({})", go_expr(&args[0], lvl));
                    }
                    "env" if args.len() == 1 => {
                        return format!("__steel_env({})", go_expr(&args[0], lvl));
                    }
                    "upper" if args.len() == 1 => {
                        return format!("strings.ToUpper(fmt.Sprint({}))", go_expr(&args[0], lvl));
                    }
                    "lower" if args.len() == 1 => {
                        return format!("strings.ToLower(fmt.Sprint({}))", go_expr(&args[0], lvl));
                    }
                    "trim" if args.len() == 1 => {
                        return format!("strings.TrimSpace(fmt.Sprint({}))", go_expr(&args[0], lvl));
                    }
                    "contains" if args.len() == 2 => {
                        return format!("__steel_contains({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "starts_with" if args.len() == 2 => {
                        return format!("strings.HasPrefix(fmt.Sprint({}), fmt.Sprint({}))", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "ends_with" if args.len() == 2 => {
                        return format!("strings.HasSuffix(fmt.Sprint({}), fmt.Sprint({}))", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "split" if args.len() == 2 => {
                        return format!("__steel_split({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "join" if args.len() == 2 => {
                        return format!("__steel_join({}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl));
                    }
                    "replace" if args.len() == 3 => {
                        return format!("strings.ReplaceAll(fmt.Sprint({}), fmt.Sprint({}), fmt.Sprint({}))", go_expr(&args[0], lvl), go_expr(&args[1], lvl), go_expr(&args[2], lvl));
                    }
                    "reverse" if args.len() == 1 => {
                        return format!("__steel_reverse({})", go_expr(&args[0], lvl));
                    }
                    "keys" if args.len() == 1 => {
                        return format!("__steel_keys({})", go_expr(&args[0], lvl));
                    }
                    "values" if args.len() == 1 => {
                        return format!("__steel_values({})", go_expr(&args[0], lvl));
                    }
                    "slice" if args.len() >= 2 => {
                        let end_e = args.get(2).map(|a| go_expr(a, lvl)).unwrap_or_else(|| "nil".to_string());
                        return format!("__steel_slice({}, {}, {})", go_expr(&args[0], lvl), go_expr(&args[1], lvl), end_e);
                    }
                    "sort" if args.len() == 1 => {
                        return format!("__steel_sort({})", go_expr(&args[0], lvl));
                    }
                    _ => {}
                }
            }
            let arg_strs: Vec<String> = args.iter().map(|a| go_expr(a, lvl)).collect();
            if arg_strs.is_empty() {
                format!("__steel_call({})", go_expr(callee, lvl))
            } else {
                format!("__steel_call({}, {})", go_expr(callee, lvl), arg_strs.join(", "))
            }
        }
        Expr::Index { target, index } => format!(
            "__steel_get({}, {})",
            go_expr(target, lvl),
            go_expr(index, lvl)
        ),
        Expr::Member { target, field } => format!(
            "{}.(map[string]any)[\"{}\"]",
            go_expr(target, lvl),
            escape_str(field)
        ),
        Expr::FnExpr { params, body } => {
            let mut s = String::from("func(__args ...any) any {\n");
            let inner_pad = indent(lvl + 1);
            for (i, (p, def)) in params.iter().enumerate() {
                if let Some(def_expr) = def {
                    s.push_str(&format!(
                        "{}var {} any = {}; if len(__args) > {} {{ {} = __args[{}] }}; _ = {}\n",
                        inner_pad, p, go_expr(def_expr, lvl + 1), i, p, i, p
                    ));
                } else {
                    s.push_str(&format!(
                        "{}var {} any; if len(__args) > {} {{ {} = __args[{}] }}; _ = {}\n",
                        inner_pad, p, i, p, i, p
                    ));
                }
            }
            for st in body {
                s.push_str(&go_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}    return nil\n{}}}", indent(lvl), indent(lvl)));
            s
        }
        Expr::Range { start, end } => format!(
            "__steel_range({}, {})",
            go_expr(start, lvl),
            go_expr(end, lvl)
        ),
        Expr::Spread(inner) => go_expr(inner, lvl),
    }
}

// MULTI-FILE PROJECT TRANSPILATION (Default non-bundled directory output)

pub struct ModuleUnit {
    pub name: String,
    pub is_entry: bool,
    pub imports: Vec<String>,
    pub program: Program,
}

fn module_exported_fns(program: &Program) -> Vec<(String, Vec<(String, Option<Expr>)>)> {
    let mut out = Vec::new();
    for stmt in &program.statements {
        match stmt {
            Stmt::FnDecl { name, params, .. } => {
                out.push((name.clone(), params.clone()));
            }
            Stmt::ClassDecl {
                name,
                parent,
                methods,
            } => {
                if let Stmt::FnDecl { name, params, .. } = desugar_class(name, parent, methods) {
                    out.push((name, params));
                }
            }
            _ => {}
        }
    }
    out
}

/// Transpiles a multi-file Steel project into separate target language files
/// (mirroring the original module structure instead of combining into one file).
pub fn transpile_project(modules: &[ModuleUnit], target: Target) -> Vec<(String, String)> {
    let opt_modules: Vec<ModuleUnit> = modules
        .iter()
        .map(|m| ModuleUnit {
            name: m.name.clone(),
            is_entry: m.is_entry,
            imports: m.imports.clone(),
            program: crate::optimizer::optimize_program(&m.program),
        })
        .collect();

    match target {
        Target::C => emit_c_project(&opt_modules, target),
        Target::Cpp => crate::codegen_cpp::emit_cpp_project(&opt_modules),
        Target::Rust => emit_rust_project(&opt_modules),
        Target::Python => emit_python_project(&opt_modules),
        Target::Js => emit_js_project(&opt_modules, target),
        Target::Ts => crate::codegen_ts::emit_ts_project(&opt_modules),
        Target::Go => emit_go_project(&opt_modules),
        Target::Lua => emit_lua_project(&opt_modules),
    }
}

fn emit_c_project(modules: &[ModuleUnit], target: Target) -> Vec<(String, String)> {
    let ext = target.extension();
    let hdr_ext = if target == Target::Cpp { "hpp" } else { "h" };
    let lang_label = target.name();

    // First pass: generate each module's lambdas, top_fns, and main_body
    let mut rendered: Vec<(String, String, String)> = Vec::new();
    let mut all_user_code = String::new();

    C_TOP_FNS.with(|f| {
        let mut set = f.borrow_mut();
        set.clear();
        for m in modules {
            for stmt in &m.program.statements {
                match stmt {
                    Stmt::FnDecl { name, .. } | Stmt::ClassDecl { name, .. } => {
                        set.insert(name.clone());
                    }
                    _ => {}
                }
            }
        }
    });
    C_CUR_LOCALS.with(|l| l.borrow_mut().clear());

    for m in modules {
        let mut lambdas = String::new();
        let mut lambda_id = 0;
        let mut top_fns = String::new();
        let mut main_body = String::new();
        let mut declared = std::collections::HashSet::new();

        for stmt in &m.program.statements {
            match stmt {
                Stmt::Import(_) => {}
                Stmt::FnDecl { name, params, body } => {
                    top_fns.push_str(&c_top_fn(name, params, body, &mut lambdas, &mut lambda_id));
                }
                Stmt::ClassDecl {
                    name,
                    parent,
                    methods,
                } => {
                    if let Stmt::FnDecl { name, params, body } = desugar_class(name, parent, methods)
                    {
                        top_fns.push_str(&c_top_fn(
                            &name,
                            &params,
                            &body,
                            &mut lambdas,
                            &mut lambda_id,
                        ));
                    }
                }
                other => {
                    main_body.push_str(&c_stmt(
                        other,
                        1,
                        &mut declared,
                        &mut lambdas,
                        &mut lambda_id,
                    ));
                }
            }
        }
        all_user_code.push_str(&lambdas);
        all_user_code.push_str(&top_fns);
        all_user_code.push_str(&main_body);
        rendered.push((lambdas, top_fns, main_body));
    }

    let needs_arr = all_user_code.contains("sv_arr_new")
        || all_user_code.contains("sv_push")
        || all_user_code.contains("sv_pop")
        || all_user_code.contains("sv_range")
        || all_user_code.contains(".arr")
        || all_user_code.contains("sv_get")
        || all_user_code.contains("sv_set")
        || all_user_code.contains("sv_map_hof")
        || all_user_code.contains("sv_filter_hof")
        || all_user_code.contains("sv_reduce_hof");
    let needs_map = all_user_code.contains("sv_map_new")
        || all_user_code.contains("sv_map_set")
        || all_user_code.contains("sv_map_get")
        || all_user_code.contains("sv_get")
        || all_user_code.contains("sv_set");
    let needs_add = all_user_code.contains("sv_add(");
    let needs_eq = all_user_code.contains("sv_eq(");
    let needs_truthy = all_user_code.contains("sv_truthy(");
    let needs_len = all_user_code.contains("sv_len(");
    let needs_hof = all_user_code.contains("sv_map_hof")
        || all_user_code.contains("sv_filter_hof")
        || all_user_code.contains("sv_reduce_hof");
    let needs_arena = needs_arr || needs_map || needs_add;

    // Build shared steel_rt.h / steel_rt.hpp
    let mut rt = format!(
        "/* Steel Shared Runtime Header (Target: {}) */\n\
         #ifndef STEEL_RT_H\n\
         #define STEEL_RT_H\n\n\
         #include <stdio.h>\n\
         #include <stdlib.h>\n\
         #include <string.h>\n\
         #include <stdbool.h>\n\
         #include <stdarg.h>\n\
         #include <math.h>\n\n",
        lang_label
    );

    if !needs_arr && !needs_map && !needs_hof {
        rt.push_str(
            "typedef enum { ST_NULL, ST_BOOL, ST_NUM, ST_STR } SType;\n\
             typedef struct { SType type; double num; const char* str; } SVal;\n\
             static inline SVal sv_null(void) { return (SVal){ST_NULL, 0, NULL}; }\n\
             static inline SVal sv_bool(bool b) { return (SVal){ST_BOOL, b ? 1.0 : 0.0, NULL}; }\n\
             static inline SVal sv_num(double n) { return (SVal){ST_NUM, n, NULL}; }\n\
             static inline SVal sv_str(const char* s) { return (SVal){ST_STR, 0, s}; }\n",
        );
        if needs_truthy {
            rt.push_str(
                "static inline bool sv_truthy(SVal v) {\n\
                 \x20   return v.type == ST_STR ? (v.str && v.str[0] != '\\0') : (v.type != ST_NULL && v.num != 0.0);\n\
                 }\n",
            );
        }
        if needs_eq {
            rt.push_str(
                "static inline bool sv_eq(SVal a, SVal b) {\n\
                 \x20   if (a.type != b.type) return false;\n\
                 \x20   return a.type == ST_STR ? strcmp(a.str, b.str) == 0 : a.num == b.num;\n\
                 }\n",
            );
        }
        if needs_add || all_user_code.contains("sv_str_cast(") {
            rt.push_str(
                "static inline SVal sv_str_cast(SVal v) {\n\
                 \x20   if (v.type == ST_STR) return v;\n\
                 \x20   char* b = (char*)calloc(1, 64);\n\
                 \x20   if (v.type == ST_NULL) snprintf(b, 64, \"null\");\n\
                 \x20   else if (v.type == ST_BOOL) snprintf(b, 64, \"%s\", v.num != 0.0 ? \"true\" : \"false\");\n\
                 \x20   else snprintf(b, 64, floor(v.num) == v.num ? \"%.0f\" : \"%g\", v.num);\n\
                 \x20   return sv_str(b);\n\
                 }\n",
            );
        }
        if needs_add {
            rt.push_str(
                "static inline SVal sv_add(SVal a, SVal b) {\n\
                 \x20   if (a.type == ST_NUM && b.type == ST_NUM) return sv_num(a.num + b.num);\n\
                 \x20   SVal sa = sv_str_cast(a), sb = sv_str_cast(b);\n\
                 \x20   char* r = (char*)calloc(1, strlen(sa.str) + strlen(sb.str) + 1);\n\
                 \x20   strcpy(r, sa.str); strcat(r, sb.str);\n\
                 \x20   return sv_str(r);\n\
                 }\n",
            );
        }
        if all_user_code.contains("sv_print_one(") {
            rt.push_str(
                "static inline void sv_print_one(SVal v) {\n\
                 \x20   if (v.type == ST_STR) printf(\"%s\", v.str);\n\
                 \x20   else if (v.type == ST_NUM) printf(floor(v.num) == v.num ? \"%.0f\" : \"%g\", v.num);\n\
                 \x20   else if (v.type == ST_BOOL) printf(\"%s\", v.num != 0.0 ? \"true\" : \"false\");\n\
                 \x20   else printf(\"null\");\n\
                 }\n",
            );
        }
    } else {
        rt.push_str(
            "typedef enum { ST_NULL, ST_BOOL, ST_NUM, ST_STR, ST_ARR, ST_MAP, ST_FN } SType;\n\
             typedef struct SVal SVal;\n\
             typedef struct SArr SArr;\n\
             typedef struct SMap SMap;\n\
             typedef SVal (*SCFn)(SVal __st_this, int argc, SVal* argv);\n\
             struct SVal { SType type; SMap* env; union { double num; const char* str; SArr* arr; SMap* map; SCFn fn; }; };\n\
             struct SArr { SVal* data; int len; int cap; };\n\
             struct SMap { const char** keys; SVal* vals; int len; int cap; };\n\
             static inline void* sv_alloc(size_t sz) { return calloc(1, sz); }\n\
             static inline void sv_cleanup(void) {}\n\
             static inline SVal sv_null(void) { SVal v; v.type = ST_NULL; v.num = 0; return v; }\n\
             static inline SVal sv_bool(bool b) { SVal v; v.type = ST_BOOL; v.num = b ? 1.0 : 0.0; return v; }\n\
             static inline SVal sv_num(double n) { SVal v; v.type = ST_NUM; v.num = n; return v; }\n\
             static inline SVal sv_str(const char* s) { SVal v; v.type = ST_STR; v.str = s; return v; }\n\
             static inline SVal sv_fn(SCFn f) { SVal v; v.type = ST_FN; v.env = NULL; v.fn = f; return v; }\n\
             static inline SVal sv_call_fn(SVal f, int argc, SVal* argv) {\n\
             \x20   if (f.type != ST_FN || !f.fn) return sv_null();\n\
             \x20   SVal env; env.type = f.env ? ST_MAP : ST_NULL; env.env = NULL; env.map = f.env;\n\
             \x20   return f.fn(env, argc, argv);\n\
             }\n",
        );
        if needs_arr {
            rt.push_str(
                "static inline SVal sv_arr_new(int count, SVal* init_items) {\n\
                 \x20   SArr* a = (SArr*)sv_alloc(sizeof(SArr)); a->cap = count > 4 ? count * 2 : 8; a->len = count;\n\
                 \x20   a->data = (SVal*)sv_alloc(sizeof(SVal) * a->cap);\n\
                 \x20   for (int i = 0; i < count; i++) a->data[i] = init_items[i];\n\
                 \x20   SVal v; v.type = ST_ARR; v.arr = a; return v;\n\
                 }\n\
                 static inline SVal sv_push(SVal arr, SVal item) {\n\
                 \x20   if (arr.type != ST_ARR) return arr;\n\
                 \x20   SArr* a = arr.arr;\n\
                 \x20   if (a->len >= a->cap) { int nc = a->cap * 2; SVal* nd = (SVal*)sv_alloc(sizeof(SVal) * nc); memcpy(nd, a->data, sizeof(SVal) * a->len); a->data = nd; a->cap = nc; }\n\
                 \x20   a->data[a->len++] = item; return arr;\n\
                 }\n\
                 static inline SVal sv_pop(SVal arr) { return (arr.type != ST_ARR || arr.arr->len == 0) ? sv_null() : arr.arr->data[--arr.arr->len]; }\n\
                 static inline SVal sv_range(SVal a, SVal b) { int s = (int)a.num, e = (int)b.num; SVal r = sv_arr_new(0, NULL); for (int i = s; i < e; i++) sv_push(r, sv_num(i)); return r; }\n",
            );
        }
        if needs_map {
            rt.push_str(
                "static inline SVal sv_map_new(void) {\n\
                 \x20   SMap* m = (SMap*)sv_alloc(sizeof(SMap)); m->cap = 16; m->len = 0;\n\
                 \x20   m->keys = (const char**)sv_alloc(sizeof(char*) * m->cap); m->vals = (SVal*)sv_alloc(sizeof(SVal) * m->cap);\n\
                 \x20   SVal v; v.type = ST_MAP; v.map = m; return v;\n\
                 }\n\
                 static inline void sv_map_set(SVal obj, const char* key, SVal val) {\n\
                 \x20   if (obj.type != ST_MAP) return;\n\
                 \x20   SMap* m = obj.map;\n\
                 \x20   for (int i = 0; i < m->len; i++) if (strcmp(m->keys[i], key) == 0) { m->vals[i] = val; return; }\n\
                 \x20   if (m->len >= m->cap) { int nc = m->cap * 2; const char** nk = (const char**)sv_alloc(sizeof(char*) * nc); SVal* nv = (SVal*)sv_alloc(sizeof(SVal) * nc); memcpy(nk, m->keys, sizeof(char*) * m->len); memcpy(nv, m->vals, sizeof(SVal) * m->len); m->keys = nk; m->vals = nv; m->cap = nc; }\n\
                 \x20   m->keys[m->len] = key; m->vals[m->len] = val; m->len++;\n\
                 }\n\
                 static inline SVal sv_map_get(SVal obj, const char* key) {\n\
                 \x20   if (obj.type != ST_MAP) return sv_null();\n\
                 \x20   for (int i = 0; i < obj.map->len; i++) if (strcmp(obj.map->keys[i], key) == 0) return obj.map->vals[i];\n\
                 \x20   return sv_null();\n\
                 }\n\
                 static inline SVal sv_call_method(SVal obj, const char* name, int argc, ...) {\n\
                 \x20   SVal m = sv_map_get(obj, name);\n\
                 \x20   if (m.type != ST_FN || !m.fn) return sv_null();\n\
                 \x20   SVal argv[16]; va_list ap; va_start(ap, argc);\n\
                 \x20   for (int i = 0; i < argc && i < 16; i++) argv[i] = va_arg(ap, SVal);\n\
                 \x20   va_end(ap); return m.fn(obj, argc, argv);\n\
                 }\n\
                 static inline SVal sv_closure(SCFn fn, int ncap, const char** ks, SVal* vs) {\n\
                 \x20   SVal env = sv_map_new();\n\
                 \x20   for (int i = 0; i < ncap; i++) sv_map_set(env, ks[i], vs[i]);\n\
                 \x20   SVal v = sv_fn(fn); v.env = env.map; return v;\n\
                 }\n",
            );
        }
        if all_user_code.contains("sv_get(") || all_user_code.contains("sv_set(") {
            rt.push_str(
                "static inline SVal sv_get(SVal t, SVal idx) {\n\
                 \x20   if (t.type == ST_MAP && idx.type == ST_STR) return sv_map_get(t, idx.str);\n\
             \x20   if (t.type == ST_ARR && idx.type == ST_NUM) { int i = (int)idx.num; if (i < 0) i += t.arr->len; return (i >= 0 && i < t.arr->len) ? t.arr->data[i] : sv_null(); }\n\
                 \x20   return sv_null();\n\
                 }\n\
                 static inline void sv_set(SVal t, SVal idx, SVal val) { if (t.type == ST_MAP && idx.type == ST_STR) { sv_map_set(t, idx.str, val); return; } if (t.type == ST_ARR && idx.type == ST_NUM) { int i = (int)idx.num; if (i < 0) i += t.arr->len; if (i >= 0 && i < t.arr->len) t.arr->data[i] = val; } }\n",
            );
        }
        rt.push_str(
            "static inline bool sv_truthy(SVal v) {\n\
             \x20   if (v.type == ST_NULL) return false;\n\
             \x20   if (v.type == ST_BOOL || v.type == ST_NUM) return v.num != 0.0;\n\
             \x20   if (v.type == ST_STR) return v.str && v.str[0] != '\\0';\n\
             \x20   return true;\n\
             }\n\
             static inline bool sv_eq(SVal a, SVal b) {\n\
             \x20   if (a.type != b.type) return false;\n\
             \x20   return a.type == ST_STR ? strcmp(a.str, b.str) == 0 : a.num == b.num;\n\
             }\n\
             static inline SVal sv_str_cast(SVal v) {\n\
             \x20   if (v.type == ST_STR) return v;\n\
             \x20   char* buf = (char*)sv_alloc(128);\n\
             \x20   if (v.type == ST_NULL) snprintf(buf, 128, \"null\");\n\
             \x20   else if (v.type == ST_BOOL) snprintf(buf, 128, \"%s\", v.num != 0.0 ? \"true\" : \"false\");\n\
             \x20   else snprintf(buf, 128, floor(v.num) == v.num ? \"%.0f\" : \"%g\", v.num);\n\
             \x20   return sv_str(buf);\n\
             }\n\
             static inline SVal sv_add(SVal a, SVal b) {\n\
             \x20   if (a.type == ST_NUM && b.type == ST_NUM) return sv_num(a.num + b.num);\n\
             \x20   SVal sa = sv_str_cast(a), sb = sv_str_cast(b);\n\
             \x20   char* res = (char*)sv_alloc(strlen(sa.str) + strlen(sb.str) + 1);\n\
             \x20   strcpy(res, sa.str); strcat(res, sb.str);\n\
             \x20   return sv_str(res);\n\
             }\n",
        );
        if needs_len {
            rt.push_str(
                "static inline SVal sv_len(SVal v) {\n\
                 \x20   if (v.type == ST_ARR) return sv_num(v.arr->len);\n\
                 \x20   if (v.type == ST_STR) return sv_num((double)strlen(v.str));\n\
                 \x20   if (v.type == ST_MAP) return sv_num(v.map->len);\n\
                 \x20   return sv_num(0);\n\
                 }\n",
            );
        }
        if needs_hof {
            rt.push_str(
                "static inline SVal sv_map_hof(SVal arr, SVal fn) { SVal out = sv_arr_new(0, NULL); SVal env; env.type = fn.env ? ST_MAP : ST_NULL; env.env = NULL; env.map = fn.env; for (int i = 0; i < arr.arr->len; i++) sv_push(out, fn.fn(env, 1, &arr.arr->data[i])); return out; }\n\
                 static inline SVal sv_filter_hof(SVal arr, SVal fn) { SVal out = sv_arr_new(0, NULL); SVal env; env.type = fn.env ? ST_MAP : ST_NULL; env.env = NULL; env.map = fn.env; for (int i = 0; i < arr.arr->len; i++) if (sv_truthy(fn.fn(env, 1, &arr.arr->data[i]))) sv_push(out, arr.arr->data[i]); return out; }\n\
                 static inline SVal sv_reduce_hof(SVal arr, SVal fn, SVal init) { SVal acc = init; SVal env; env.type = fn.env ? ST_MAP : ST_NULL; env.env = NULL; env.map = fn.env; for (int i = 0; i < arr.arr->len; i++) { SVal a[2] = { acc, arr.arr->data[i] }; acc = fn.fn(env, 2, a); } return acc; }\n",
            );
        }
        rt.push_str(
            "static inline void sv_print_one(SVal v) {\n\
             \x20   if (v.type == ST_ARR) { printf(\"[\"); for (int i = 0; i < v.arr->len; i++) { if (i > 0) printf(\", \"); sv_print_one(v.arr->data[i]); } printf(\"]\"); }\n\
             \x20   else printf(\"%s\", sv_str_cast(v).str);\n\
             }\n",
        );
    }
    rt.push_str(&c_extra_stdlib(&all_user_code));
    rt.push_str("\n#endif /* STEEL_RT_H */\n");

    let mut files = Vec::new();
    files.push((format!("steel_rt.{}", hdr_ext), rt));

    for (idx, m) in modules.iter().enumerate() {
        let (lambdas, top_fns, main_body) = &rendered[idx];
        let fns = module_exported_fns(&m.program);

        if !m.is_entry {
            let guard = format!("{}_H", m.name.to_uppercase().replace('-', "_"));
            let mut hdr = format!(
                "/* Header for module '{}' (Target: {}) */\n\
                 #ifndef {}\n\
                 #define {}\n\n\
                 #include \"steel_rt.{}\"\n\n",
                m.name, lang_label, guard, guard, hdr_ext
            );
            for (fn_name, params) in &fns {
                let p_sig: Vec<String> = params.iter().map(|(p, _)| format!("SVal {}", c_safe_ident(p))).collect();
                let sig = if p_sig.is_empty() {
                    "void".to_string()
                } else {
                    p_sig.join(", ")
                };
                hdr.push_str(&format!("SVal {}({});\n", c_safe_ident(fn_name), sig));
                hdr.push_str(&format!("SVal __st_fn_{}(SVal, int, SVal*);\n", c_safe_ident(fn_name)));
            }
            hdr.push_str(&format!("\n#endif /* {} */\n", guard));
            files.push((format!("{}.{}", m.name, hdr_ext), hdr));
        }

        let mut src = format!(
            "/* Module '{}' (Target: {}) */\n",
            m.name, lang_label
        );
        if m.is_entry {
            src.push_str(&format!("#include \"steel_rt.{}\"\n", hdr_ext));
        } else {
            src.push_str(&format!("#include \"{}.{}\"\n", m.name, hdr_ext));
        }
        let mut included = std::collections::HashSet::new();
        for imp in &m.imports {
            if imp != &m.name && included.insert(imp.clone()) {
                src.push_str(&format!("#include \"{}.{}\"\n", imp, hdr_ext));
            }
        }
        // Also auto-include sibling module headers if this module calls their functions
        let mod_body = format!("{}{}{}", lambdas, top_fns, main_body);
        for other in modules {
            if other.name == m.name || other.is_entry || included.contains(&other.name) {
                continue;
            }
            let other_fns = module_exported_fns(&other.program);
            if other_fns.iter().any(|(fn_name, _)| mod_body.contains(&format!("{}(", fn_name))) {
                included.insert(other.name.clone());
                src.push_str(&format!("#include \"{}.{}\"\n", other.name, hdr_ext));
            }
        }
        src.push('\n');
        src.push_str(lambdas);
        src.push_str(top_fns);
        if m.is_entry {
            src.push_str("int main(void) {\n");
            src.push_str(main_body);
            if needs_arena && (needs_arr || needs_map || needs_hof) {
                src.push_str("    sv_cleanup();\n");
            }
            src.push_str("    return 0;\n}\n");
        }
        files.push((format!("{}.{}", m.name, ext), src));
    }

    files
}

fn emit_rust_project(modules: &[ModuleUnit]) -> Vec<(String, String)> {
    let mut files = Vec::new();

    // Shared steel_rt.rs
    let rt = "// Steel Shared Runtime (Target: Rust)\n\
         #![allow(unused_variables,unused_mut,dead_code,unreachable_code,unused_parens,non_snake_case)]\n\
         use std::cell::RefCell;\n\
         use std::collections::BTreeMap;\n\
         use std::fmt;\n\
         use std::ops::{Add, Sub, Mul, Div, Rem, Neg, Not};\n\
         use std::rc::Rc;\n\n\
         #[derive(Clone)]\n\
         pub enum SVal {\n\
         \x20   Null,\n\
         \x20   Bool(bool),\n\
         \x20   Num(f64),\n\
         \x20   Str(String),\n\
         \x20   Arr(Rc<RefCell<Vec<SVal>>>),\n\
         \x20   Map(Rc<RefCell<BTreeMap<String, SVal>>>),\n\
         \x20   Func(Rc<dyn Fn(Vec<SVal>) -> SVal>),\n\
         }\n\n\
         impl SVal {\n\
         \x20   pub fn is_truthy(&self) -> bool {\n\
         \x20       match self {\n\
         \x20           SVal::Null => false,\n\
         \x20           SVal::Bool(b) => *b,\n\
         \x20           SVal::Num(n) => *n != 0.0,\n\
         \x20           SVal::Str(s) => !s.is_empty(),\n\
         \x20           SVal::Arr(a) => !a.borrow().is_empty(),\n\
         \x20           SVal::Map(m) => !m.borrow().is_empty(),\n\
         \x20           SVal::Func(_) => true,\n\
         \x20       }\n\
         \x20   }\n\
         \x20   pub fn as_num(&self) -> f64 {\n\
         \x20       match self { SVal::Num(n) => *n, SVal::Bool(true) => 1.0, SVal::Str(s) => s.trim().parse().unwrap_or(0.0), _ => 0.0 }\n\
         \x20   }\n\
         \x20   pub fn call(&self, args: Vec<SVal>) -> SVal {\n\
         \x20       if let SVal::Func(f) = self { f(args) } else { panic!(\"Attempted to call non-function: {}\", self) }\n\
         \x20   }\n\
         \x20   pub fn get(&self, idx: SVal) -> SVal {\n\
         \x20       match (self, idx) {\n\
         \x20           (SVal::Arr(a), SVal::Num(n)) => { let b = a.borrow(); let mut i = n as isize; if i < 0 { i += b.len() as isize; } if i >= 0 { b.get(i as usize).cloned().unwrap_or(SVal::Null) } else { SVal::Null } },\n\
         \x20           (SVal::Map(m), SVal::Str(k)) => m.borrow().get(&k).cloned().unwrap_or(SVal::Null),\n\
         \x20           _ => SVal::Null,\n\
         \x20       }\n\
         \x20   }\n\
         \x20   pub fn set(&self, idx: SVal, val: SVal) {\n\
         \x20       match (self, idx) {\n\
         \x20           (SVal::Arr(a), SVal::Num(n)) => { let mut b = a.borrow_mut(); let mut i = n as isize; if i < 0 { i += b.len() as isize; } if i >= 0 && (i as usize) < b.len() { b[i as usize] = val; } }\n\
         \x20           (SVal::Map(m), SVal::Str(k)) => { m.borrow_mut().insert(k, val); }\n\
         \x20           _ => {}\n\
         \x20       }\n\
         \x20   }\n\
         \x20   pub fn iter_vec(&self) -> Vec<SVal> {\n\
         \x20       match self { SVal::Arr(a) => a.borrow().clone(), _ => vec![] }\n\
         \x20   }\n\
         }\n\n\
         impl fmt::Display for SVal {\n\
         \x20   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n\
         \x20       match self {\n\
         \x20           SVal::Null => write!(f, \"null\"),\n\
         \x20           SVal::Bool(b) => write!(f, \"{}\", b),\n\
         \x20           SVal::Num(n) => if n.fract() == 0.0 { write!(f, \"{:.0}\", n) } else { write!(f, \"{}\", n) },\n\
         \x20           SVal::Str(s) => write!(f, \"{}\", s),\n\
         \x20           SVal::Arr(a) => {\n\
         \x20               let items: Vec<String> = a.borrow().iter().map(|v| v.to_string()).collect();\n\
         \x20               write!(f, \"[{}]\", items.join(\", \"))\n\
         \x20           }\n\
         \x20           SVal::Map(m) => {\n\
         \x20               let items: Vec<String> = m.borrow().iter().map(|(k, v)| format!(\"{}: {}\", k, v)).collect();\n\
         \x20               write!(f, \"{{{}}}\", items.join(\", \"))\n\
         \x20           }\n\
         \x20           SVal::Func(_) => write!(f, \"<fn>\"),\n\
         \x20       }\n\
         \x20   }\n\
         }\n\n\
         impl PartialEq for SVal {\n\
         \x20   fn eq(&self, other: &Self) -> bool {\n\
         \x20       match (self, other) {\n\
         \x20           (SVal::Null, SVal::Null) => true,\n\
         \x20           (SVal::Bool(a), SVal::Bool(b)) => a == b,\n\
         \x20           (SVal::Num(a), SVal::Num(b)) => a == b,\n\
         \x20           (SVal::Str(a), SVal::Str(b)) => a == b,\n\
         \x20           _ => false,\n\
         \x20       }\n\
         \x20   }\n\
         }\n\n\
         impl Add for SVal { type Output = SVal; fn add(self, rhs: SVal) -> SVal {\n\
         \x20   match (&self, &rhs) {\n\
         \x20       (SVal::Str(_), _) | (_, SVal::Str(_)) => SVal::Str(format!(\"{}{}\", self, rhs)),\n\
         \x20       _ => SVal::Num(self.as_num() + rhs.as_num()),\n\
         \x20   }\n\
         }}\n\
         impl Sub for SVal { type Output = SVal; fn sub(self, rhs: SVal) -> SVal { SVal::Num(self.as_num() - rhs.as_num()) } }\n\
         impl Mul for SVal { type Output = SVal; fn mul(self, rhs: SVal) -> SVal { SVal::Num(self.as_num() * rhs.as_num()) } }\n\
         impl Div for SVal { type Output = SVal; fn div(self, rhs: SVal) -> SVal { SVal::Num(self.as_num() / rhs.as_num()) } }\n\
         impl Rem for SVal { type Output = SVal; fn rem(self, rhs: SVal) -> SVal { SVal::Num(self.as_num() % rhs.as_num()) } }\n\
         impl Neg for SVal { type Output = SVal; fn neg(self) -> SVal { SVal::Num(-self.as_num()) } }\n\
         impl Not for SVal { type Output = SVal; fn not(self) -> SVal { SVal::Bool(!self.is_truthy()) } }\n\n\
         pub fn __steel_range(a: SVal, b: SVal) -> SVal {\n\
         \x20   let start = a.as_num() as i64;\n\
         \x20   let end = b.as_num() as i64;\n\
         \x20   let v: Vec<SVal> = (start..end).map(|i| SVal::Num(i as f64)).collect();\n\
         \x20   SVal::Arr(Rc::new(RefCell::new(v)))\n\
         }\n";
    files.push(("steel_rt.rs".to_string(), rt.to_string()));

    // Collect all top-level function names across ALL modules so cross-module calls emit direct calls
    let mut all_fn_names = std::collections::HashSet::new();
    for m in modules {
        for (name, _) in module_exported_fns(&m.program) {
            all_fn_names.insert(name);
        }
    }

    for m in modules {
        let mut out = format!(
            "// Module '{}' (Target: Rust)\n\
             #![allow(unused_variables,unused_mut,dead_code,unreachable_code,unused_parens,non_snake_case,unused_imports)]\n\
             use std::cell::RefCell;\n\
             use std::collections::BTreeMap;\n\
             use std::rc::Rc;\n",
            m.name
        );

        if m.is_entry {
            out.push_str("mod steel_rt;\n");
            for other in modules {
                if !other.is_entry {
                    out.push_str(&format!("mod {};\n", other.name));
                }
            }
        }
        out.push_str("use crate::steel_rt::*;\n");
        let mut used_mods = std::collections::HashSet::new();
        for imp in &m.imports {
            if imp != &m.name && used_mods.insert(imp.clone()) {
                out.push_str(&format!("use crate::{}::*;\n", imp));
            }
        }
        for other in modules {
            if !other.is_entry && other.name != m.name && used_mods.insert(other.name.clone()) {
                out.push_str(&format!("use crate::{}::*;\n", other.name));
            }
        }
        out.push('\n');

        let mut top_fns = String::new();
        let mut main_body = String::new();
        for stmt in &m.program.statements {
            match stmt {
                Stmt::Import(_) => {}
                Stmt::FnDecl { name, params, body } => {
                    let f = rs_top_fn(name, params, body, &all_fn_names);
                    if !m.is_entry {
                        top_fns.push_str(&f.replacen("fn ", "pub fn ", 1).replace("#[allow(dead_code)] fn ", "#[allow(dead_code)] pub fn "));
                    } else {
                        top_fns.push_str(&f);
                    }
                }
                Stmt::ClassDecl {
                    name,
                    parent,
                    methods,
                } => {
                    if let Stmt::FnDecl { name, params, body } = desugar_class(name, parent, methods)
                    {
                        let f = rs_top_fn(&name, &params, &body, &all_fn_names);
                        if !m.is_entry {
                            top_fns.push_str(&f.replacen("fn ", "pub fn ", 1).replace("#[allow(dead_code)] fn ", "#[allow(dead_code)] pub fn "));
                        } else {
                            top_fns.push_str(&f);
                        }
                    }
                }
                other => {
                    main_body.push_str(&rs_stmt(other, 1, &all_fn_names));
                }
            }
        }

        out.push_str(&top_fns);
        if m.is_entry {
            out.push_str("fn main() {\n");
            out.push_str(&rs_builtins_preamble(&main_body));
            out.push_str(&main_body);
            out.push_str("}\n");
        }
        files.push((format!("{}.rs", m.name), out));
    }

    files
}

fn emit_python_project(modules: &[ModuleUnit]) -> Vec<(String, String)> {
    let mut files = Vec::new();
    for m in modules {
        let mut code = emit_python(&m.program);
        for imp in &m.imports {
            code = code.replace(
                &format!("# import \"{}.steel\"", imp),
                &format!("from {} import *", imp),
            );
            code = code.replace(
                &format!("# import \"{}\"", imp),
                &format!("from {} import *", imp),
            );
        }
        files.push((format!("{}.py", m.name), code));
    }
    files
}

fn emit_js_project(modules: &[ModuleUnit], target: Target) -> Vec<(String, String)> {
    let ext = target.extension();
    let mut exports_map = std::collections::HashMap::new();
    for m in modules {
        let fns: Vec<String> = module_exported_fns(&m.program)
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        exports_map.insert(m.name.clone(), fns);
    }

    let mut files = Vec::new();
    for m in modules {
        let mut code = if target == Target::Ts {
            emit_js(&m.program)
        } else {
            emit_js(&m.program)
        };
        for imp in &m.imports {
            let imp_spec = if target == Target::Ts {
                format!("./{}.ts", imp)
            } else {
                format!("./{}", imp)
            };
            let import_line = if let Some(fns) = exports_map.get(imp) {
