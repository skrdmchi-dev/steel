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
