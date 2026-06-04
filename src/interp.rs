use crate::ast::{AssignTarget, BinaryOp, ClassMethod, Expr, Program, Stmt, UnaryOp};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Rc<RefCell<Vec<Value>>>),
    Map(Rc<RefCell<BTreeMap<String, Value>>>),
    Func {
        params: Vec<(String, Option<Expr>)>,
        body: Vec<Stmt>,
        closure: Rc<RefCell<Env>>,
    },
    Class {
        name: String,
        parent: Option<String>,
        methods: Vec<ClassMethod>,
        closure: Rc<RefCell<Env>>,
    },
    BoundMethod {
        receiver: Box<Value>,
        params: Vec<(String, Option<Expr>)>,
        body: Vec<Stmt>,
        closure: Rc<RefCell<Env>>,
    },
    NativeFn(fn(Vec<Value>) -> Result<Value, String>),
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Num(n) => *n != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::Arr(a) => !a.borrow().is_empty(),
            Value::Map(m) => !m.borrow().is_empty(),
            _ => true,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Num(_) => "number",
            Value::Str(_) => "string",
            Value::Arr(_) => "array",
            Value::Map(_) => "map",
            Value::Class { .. } => "class",
            Value::Func { .. } | Value::BoundMethod { .. } | Value::NativeFn(_) => "function",
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Num(a), Value::Num(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Arr(a), Value::Arr(b)) => *a.borrow() == *b.borrow(),
            (Value::Map(a), Value::Map(b)) => *a.borrow() == *b.borrow(),
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Num(n) => {
                if n.fract() == 0.0 {
                    write!(f, "{:.0}", n)
                } else {
                    write!(f, "{}", n)
                }
            }
            Value::Str(s) => write!(f, "{}", s),
            Value::Arr(a) => {
                let items: Vec<String> = a.borrow().iter().map(|v| v.to_string()).collect();
                write!(f, "[{}]", items.join(", "))
            }
            Value::Map(m) => {
                let items: Vec<String> = m
                    .borrow()
                    .iter()
                    .filter(|(k, _)| !k.starts_with("__"))
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                write!(f, "{{{}}}", items.join(", "))
            }
            Value::Class { name, .. } => write!(f, "<class {}>", name),
            Value::Func { .. } | Value::BoundMethod { .. } | Value::NativeFn(_) => {
                write!(f, "<fn>")
            }
        }
    }
}

#[derive(Clone)]
pub struct Env {
    vars: BTreeMap<String, Value>,
    parent: Option<Rc<RefCell<Env>>>,
}

impl Env {
    pub fn new(parent: Option<Rc<RefCell<Env>>>) -> Self {
        Self {
            vars: BTreeMap::new(),
            parent,
        }
    }

    pub fn define(&mut self, name: String, val: Value) {
        self.vars.insert(name, val);
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(v) = self.vars.get(name) {
            return Some(v.clone());
        }
        if let Some(p) = &self.parent {
            return p.borrow().get(name);
        }
        None
    }

    fn exists_in_chain(&self, name: &str) -> bool {
        if self.vars.contains_key(name) {
            return true;
        }
        if let Some(p) = &self.parent {
            return p.borrow().exists_in_chain(name);
        }
        false
    }

    pub fn assign(&mut self, name: &str, val: Value) -> Result<(), String> {
        if self.vars.contains_key(name) {
            self.vars.insert(name.to_string(), val);
            return Ok(());
        }
        if let Some(p) = &self.parent {
            if p.borrow().exists_in_chain(name) {
                return p.borrow_mut().assign(name, val);
            }
        }
        // Optional `let`: first assignment to a new variable creates it in the current scope!
        self.vars.insert(name.to_string(), val);
        Ok(())
    }
}

enum Signal {
    None,
    Return(Value),
    Break,
    Continue,
}

pub struct Interpreter {
    pub global: Rc<RefCell<Env>>,
}

impl Interpreter {
    pub fn new() -> Self {
        let mut env = Env::new(None);

        // --- Core I/O & Inspection ---
        env.define(
            "print".to_string(),
            Value::NativeFn(|args| {
                let out: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                println!("{}", out.join(" "));
                Ok(Value::Null)
            }),
        );
        env.define(
            "println".to_string(),
            Value::NativeFn(|args| {
                let out: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                println!("{}", out.join(" "));
                Ok(Value::Null)
            }),
        );
        env.define(
            "input".to_string(),
            Value::NativeFn(|args| {
                if let Some(prompt) = args.first() {
                    print!("{}", prompt);
                    let _ = io::stdout().flush();
                }
                let mut buf = String::new();
                io::stdin()
                    .read_line(&mut buf)
                    .map_err(|e| format!("input error: {}", e))?;
                Ok(Value::Str(buf.trim_end_matches(&['\r', '\n'][..]).to_string()))
            }),
        );
        env.define(
            "assert".to_string(),
            Value::NativeFn(|args| {
                let cond = args.first().map(|v| v.is_truthy()).unwrap_or(false);
                if !cond {
                    let msg = args
                        .get(1)
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "Assertion failed".to_string());
                    Err(msg)
                } else {
                    Ok(Value::Bool(true))
                }
            }),
        );
        env.define(
            "type".to_string(),
            Value::NativeFn(|args| {
                let t = args.first().map(|v| v.type_name()).unwrap_or("null");
                Ok(Value::Str(t.to_string()))
            }),
        );
        env.define(
            "str".to_string(),
            Value::NativeFn(|args| {
                Ok(Value::Str(
                    args.first().cloned().unwrap_or(Value::Null).to_string(),
                ))
            }),
        );
        env.define(
            "num".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Num(n)) => Ok(Value::Num(*n)),
                Some(Value::Str(s)) => s
                    .trim()
                    .parse::<f64>()
                    .map(Value::Num)
                    .map_err(|_| format!("Cannot convert '{}' to number", s)),
                Some(Value::Bool(b)) => Ok(Value::Num(if *b { 1.0 } else { 0.0 })),
                _ => Ok(Value::Num(0.0)),
            }),
        );
        env.define(
            "int".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Num(n)) => Ok(Value::Num(n.trunc())),
                Some(Value::Str(s)) => s
                    .trim()
                    .parse::<f64>()
                    .map(|n| Value::Num(n.trunc()))
                    .map_err(|_| format!("Cannot convert '{}' to int", s)),
                Some(Value::Bool(b)) => Ok(Value::Num(if *b { 1.0 } else { 0.0 })),
                _ => Ok(Value::Num(0.0)),
            }),
        );
        env.define(
            "bool".to_string(),
            Value::NativeFn(|args| {
                Ok(Value::Bool(
                    args.first().map(|v| v.is_truthy()).unwrap_or(false),
                ))
            }),
        );

        // --- Collections & Strings ---
        env.define(
            "len".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Arr(a)) => Ok(Value::Num(a.borrow().len() as f64)),
                Some(Value::Str(s)) => Ok(Value::Num(s.chars().count() as f64)),
                Some(Value::Map(m)) => Ok(Value::Num(m.borrow().len() as f64)),
                _ => Ok(Value::Num(0.0)),
            }),
        );
        env.define(
            "push".to_string(),
            Value::NativeFn(|args| {
                if let (Some(Value::Arr(a)), Some(v)) = (args.first(), args.get(1)) {
                    a.borrow_mut().push(v.clone());
                    Ok(Value::Arr(a.clone()))
                } else {
                    Err("push(array, value) expects an array and a value".to_string())
                }
            }),
        );
        env.define(
            "pop".to_string(),
            Value::NativeFn(|args| {
                if let Some(Value::Arr(a)) = args.first() {
                    Ok(a.borrow_mut().pop().unwrap_or(Value::Null))
                } else {
                    Err("pop(array) expects an array".to_string())
                }
            }),
        );
        env.define(
            "keys".to_string(),
            Value::NativeFn(|args| {
                if let Some(Value::Map(m)) = args.first() {
                    let ks: Vec<Value> = m
                        .borrow()
                        .keys()
                        .filter(|k| !k.starts_with("__"))
                        .map(|k| Value::Str(k.clone()))
                        .collect();
                    Ok(Value::Arr(Rc::new(RefCell::new(ks))))
                } else {
                    Err("keys(map) expects a map".to_string())
                }
            }),
        );
        env.define(
            "values".to_string(),
            Value::NativeFn(|args| {
                if let Some(Value::Map(m)) = args.first() {
                    let vs: Vec<Value> = m
                        .borrow()
                        .iter()
                        .filter(|(k, _)| !k.starts_with("__"))
                        .map(|(_, v)| v.clone())
                        .collect();
                    Ok(Value::Arr(Rc::new(RefCell::new(vs))))
                } else {
                    Err("values(map) expects a map".to_string())
                }
            }),
        );
        env.define(
            "contains".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Arr(a)), Some(needle)) => {
                    Ok(Value::Bool(a.borrow().contains(needle)))
                }
                (Some(Value::Str(s)), Some(Value::Str(sub))) => {
                    Ok(Value::Bool(s.contains(sub.as_str())))
                }
                (Some(Value::Map(m)), Some(Value::Str(k))) => {
                    Ok(Value::Bool(m.borrow().contains_key(k)))
                }
                _ => Ok(Value::Bool(false)),
            }),
        );
        env.define(
            "range".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1), args.get(2)) {
                (Some(Value::Num(a)), Some(Value::Num(b)), step_opt) => {
                    let s = *a as i64;
                    let e = *b as i64;
                    let step = step_opt
                        .and_then(|v| if let Value::Num(n) = v { Some(*n as i64) } else { None })
                        .unwrap_or(1);
                    if step == 0 {
                        return Err("range step cannot be 0".to_string());
                    }
                    let mut v = Vec::new();
                    let mut cur = s;
                    if step > 0 {
                        while cur < e {
                            v.push(Value::Num(cur as f64));
                            cur += step;
                        }
                    } else {
                        while cur > e {
                            v.push(Value::Num(cur as f64));
                            cur += step;
                        }
                    }
                    Ok(Value::Arr(Rc::new(RefCell::new(v))))
                }
                _ => Err("range(start, end, [step]) expects numbers".to_string()),
            }),
        );
        env.define(
            "split".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Str(sep))) => {
                    let parts: Vec<Value> = if sep.is_empty() {
                        s.chars().map(|c| Value::Str(c.to_string())).collect()
                    } else {
                        s.split(sep.as_str())
                            .map(|p| Value::Str(p.to_string()))
                            .collect()
                    };
                    Ok(Value::Arr(Rc::new(RefCell::new(parts))))
                }
                _ => Err("split(str, sep) expects two strings".to_string()),
            }),
        );
        env.define(
            "join".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Arr(a)), Some(Value::Str(sep))) => {
                    let parts: Vec<String> = a.borrow().iter().map(|v| v.to_string()).collect();
                    Ok(Value::Str(parts.join(sep)))
                }
                _ => Err("join(array, sep) expects an array and a string".to_string()),
            }),
        );
        env.define(
            "trim".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Str(s)) => Ok(Value::Str(s.trim().to_string())),
                _ => Err("trim(str) expects a string".to_string()),
            }),
        );
        env.define(
            "upper".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Str(s)) => Ok(Value::Str(s.to_uppercase())),
                _ => Err("upper(str) expects a string".to_string()),
            }),
        );
        env.define(
            "lower".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Str(s)) => Ok(Value::Str(s.to_lowercase())),
                _ => Err("lower(str) expects a string".to_string()),
            }),
        );
        env.define(
            "replace".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1), args.get(2)) {
                (Some(Value::Str(s)), Some(Value::Str(from)), Some(Value::Str(to))) => {
                    Ok(Value::Str(s.replace(from.as_str(), to.as_str())))
                }
                _ => Err("replace(str, from, to) expects three strings".to_string()),
            }),
        );
        env.define(
            "slice".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1), args.get(2)) {
                (Some(Value::Arr(a)), Some(Value::Num(s)), end_opt) => {
                    let b = a.borrow();
                    let start = (*s as usize).min(b.len());
                    let end = end_opt
                        .and_then(|v| if let Value::Num(n) = v { Some(*n as usize) } else { None })
                        .unwrap_or(b.len())
                        .min(b.len());
                    let sliced = if start <= end {
                        b[start..end].to_vec()
                    } else {
                        vec![]
                    };
                    Ok(Value::Arr(Rc::new(RefCell::new(sliced))))
                }
                (Some(Value::Str(st)), Some(Value::Num(s)), end_opt) => {
                    let chars: Vec<char> = st.chars().collect();
                    let start = (*s as usize).min(chars.len());
                    let end = end_opt
                        .and_then(|v| if let Value::Num(n) = v { Some(*n as usize) } else { None })
                        .unwrap_or(chars.len())
                        .min(chars.len());
                    let sliced: String = if start <= end {
                        chars[start..end].iter().collect()
                    } else {
                        String::new()
                    };
                    Ok(Value::Str(sliced))
                }
                _ => Err("slice(arr_or_str, start, [end]) invalid arguments".to_string()),
            }),
        );
        env.define(
            "reverse".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Arr(a)) => {
                    let mut v = a.borrow().clone();
                    v.reverse();
                    Ok(Value::Arr(Rc::new(RefCell::new(v))))
                }
                Some(Value::Str(s)) => Ok(Value::Str(s.chars().rev().collect())),
                _ => Err("reverse(x) expects an array or string".to_string()),
            }),
        );
        env.define(
            "sort".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Arr(a)) => {
                    let mut v = a.borrow().clone();
                    v.sort_by(|x, y| x.to_string().cmp(&y.to_string()));
                    Ok(Value::Arr(Rc::new(RefCell::new(v))))
                }
                _ => Err("sort(array) expects an array".to_string()),
            }),
        );

        // --- Math Standard Library ---
        env.define("PI".to_string(), Value::Num(std::f64::consts::PI));
        env.define("E".to_string(), Value::Num(std::f64::consts::E));
        env.define(
            "abs".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Num(n)) => Ok(Value::Num(n.abs())),
                _ => Err("abs(n) expects a number".to_string()),
            }),
        );
        env.define(
            "floor".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Num(n)) => Ok(Value::Num(n.floor())),
                _ => Err("floor(n) expects a number".to_string()),
            }),
        );
        env.define(
            "ceil".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Num(n)) => Ok(Value::Num(n.ceil())),
                _ => Err("ceil(n) expects a number".to_string()),
            }),
        );
        env.define(
            "round".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Num(n)) => Ok(Value::Num(n.round())),
                _ => Err("round(n) expects a number".to_string()),
            }),
        );
        env.define(
            "sqrt".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Num(n)) => Ok(Value::Num(n.sqrt())),
                _ => Err("sqrt(n) expects a number".to_string()),
            }),
        );
        env.define(
            "pow".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Num(a)), Some(Value::Num(b))) => Ok(Value::Num(a.powf(*b))),
                _ => Err("pow(a, b) expects two numbers".to_string()),
            }),
        );
        env.define(
            "min".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Num(a)), Some(Value::Num(b))) => Ok(Value::Num(a.min(*b))),
                _ => Err("min(a, b) expects two numbers".to_string()),
            }),
        );
        env.define(
            "max".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Num(a)), Some(Value::Num(b))) => Ok(Value::Num(a.max(*b))),
                _ => Err("max(a, b) expects two numbers".to_string()),
            }),
        );
        env.define(
            "sin".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Num(n)) => Ok(Value::Num(n.sin())),
                _ => Err("sin(n) expects a number".to_string()),
            }),
        );
        env.define(
            "cos".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Num(n)) => Ok(Value::Num(n.cos())),
                _ => Err("cos(n) expects a number".to_string()),
            }),
        );
        env.define(
            "tan".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Num(n)) => Ok(Value::Num(n.tan())),
                _ => Err("tan(n) expects a number".to_string()),
            }),
        );
        env.define(
            "starts_with".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Str(p))) => Ok(Value::Bool(s.starts_with(p.as_str()))),
                _ => Ok(Value::Bool(false)),
            }),
        );
        env.define(
            "ends_with".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Str(p))) => Ok(Value::Bool(s.ends_with(p.as_str()))),
                _ => Ok(Value::Bool(false)),
            }),
        );

        // --- File I/O & Clock ---
        env.define(
            "read_file".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Str(path)) => fs::read_to_string(path)
                    .map(Value::Str)
                    .map_err(|e| format!("read_file('{}') failed: {}", path, e)),
                _ => Err("read_file(path) expects a string path".to_string()),
            }),
        );
        env.define(
            "write_file".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Str(path)), Some(content)) => fs::write(path, content.to_string())
                    .map(|_| Value::Bool(true))
                    .map_err(|e| format!("write_file('{}') failed: {}", path, e)),
                _ => Err("write_file(path, content) expects a path and content".to_string()),
            }),
        );
        env.define(
            "clock".to_string(),
            Value::NativeFn(|_| {
                let secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0);
                Ok(Value::Num(secs))
            }),
        );

        // --- Extended Standard Library (JSON, OS, FS, Random, Hash, Collection Helpers) ---
        env.define(
            "json_stringify".to_string(),
            Value::NativeFn(|args| {
                let v = args.first().cloned().unwrap_or(Value::Null);
                Ok(Value::Str(value_to_json(&v)))
            }),
        );
        env.define(
            "json_parse".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Str(s)) => parse_simple_json(s),
                _ => Err("json_parse(str) expects a JSON string".to_string()),
            }),
        );
        env.define(
            "args".to_string(),
            Value::NativeFn(|_| {
                let cli_args: Vec<Value> =
                    std::env::args().map(Value::Str).collect();
                Ok(Value::Arr(Rc::new(RefCell::new(cli_args))))
            }),
        );
        env.define(
            "env".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Str(key)) => Ok(std::env::var(key)
                    .map(Value::Str)
                    .unwrap_or(Value::Null)),
                _ => Err("env(key) expects a string".to_string()),
            }),
        );
        env.define(
            "set_env".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Str(k)), Some(v)) => {
                    std::env::set_var(k, v.to_string());
                    Ok(Value::Bool(true))
                }
                _ => Err("set_env(key, val) expects a string key and value".to_string()),
            }),
        );
        env.define(
            "cwd".to_string(),
            Value::NativeFn(|_| {
                let p = std::env::current_dir()
                    .map(|d| d.to_string_lossy().to_string())
                    .unwrap_or_else(|_| ".".to_string());
                Ok(Value::Str(p))
            }),
        );
        env.define(
            "exec".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Str(cmd)) => {
                    let out = if cfg!(target_os = "windows") {
                        std::process::Command::new("cmd")
                            .args(["/C", cmd])
                            .output()
                    } else {
                        std::process::Command::new("sh")
                            .args(["-c", cmd])
                            .output()
                    }
                    .map_err(|e| format!("exec('{}') failed: {}", cmd, e))?;
                    Ok(Value::Str(
                        String::from_utf8_lossy(&out.stdout).trim_end().to_string(),
                    ))
                }
                _ => Err("exec(cmd) expects a command string".to_string()),
            }),
        );
        env.define(
            "exit".to_string(),
            Value::NativeFn(|args| {
                let code = args
                    .first()
                    .and_then(|v| if let Value::Num(n) = v { Some(*n as i32) } else { None })
                    .unwrap_or(0);
                std::process::exit(code);
            }),
        );
        env.define(
            "file_exists".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Str(p)) => Ok(Value::Bool(std::path::Path::new(p).exists())),
                _ => Ok(Value::Bool(false)),
            }),
        );
        env.define(
            "list_dir".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Str(p)) => {
                    let entries = fs::read_dir(p)
                        .map_err(|e| format!("list_dir('{}') failed: {}", p, e))?;
                    let mut list = Vec::new();
                    for entry in entries.flatten() {
                        list.push(Value::Str(
                            entry.file_name().to_string_lossy().to_string(),
                        ));
                    }
                    Ok(Value::Arr(Rc::new(RefCell::new(list))))
                }
                _ => Err("list_dir(path) expects a string path".to_string()),
            }),
        );
        env.define(
            "make_dir".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Str(p)) => fs::create_dir_all(p)
                    .map(|_| Value::Bool(true))
                    .map_err(|e| format!("make_dir('{}') failed: {}", p, e)),
                _ => Err("make_dir(path) expects a string path".to_string()),
            }),
        );
        env.define(
            "remove_file".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Str(p)) => fs::remove_file(p)
                    .map(|_| Value::Bool(true))
                    .map_err(|e| format!("remove_file('{}') failed: {}", p, e)),
                _ => Err("remove_file(path) expects a string path".to_string()),
            }),
        );
        env.define(
            "append_file".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Str(p)), Some(content)) => {
                    let mut f = fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(p)
                        .map_err(|e| format!("append_file('{}') failed: {}", p, e))?;
                    f.write_all(content.to_string().as_bytes())
                        .map(|_| Value::Bool(true))
                        .map_err(|e| format!("append_file write failed: {}", e))
                }
                _ => Err("append_file(path, content) expects path and content".to_string()),
            }),
        );
        env.define(
            "sleep".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Num(ms)) => {
                    std::thread::sleep(std::time::Duration::from_millis(ms.max(0.0) as u64));
                    Ok(Value::Null)
                }
                _ => Err("sleep(ms) expects milliseconds".to_string()),
            }),
        );
        env.define(
            "random".to_string(),
            Value::NativeFn(|_| Ok(Value::Num(fast_rand_f64()))),
        );
        env.define(
            "rand_int".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Num(a)), Some(Value::Num(b))) => {
                    let low = *a as i64;
                    let high = *b as i64;
                    if high <= low {
                        return Ok(Value::Num(low as f64));
                    }
                    let span = (high - low) as f64;
                    let r = low + (fast_rand_f64() * span).floor() as i64;
                    Ok(Value::Num(r as f64))
                }
                _ => Err("rand_int(min, max) expects two numbers".to_string()),
            }),
        );
        env.define(
            "hash".to_string(),
            Value::NativeFn(|args| {
                let s = args.first().map(|v| v.to_string()).unwrap_or_default();
                let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                for b in s.as_bytes() {
                    h ^= *b as u64;
                    h = h.wrapping_mul(0x0100_0000_01b3);
                }
                Ok(Value::Str(format!("{:016x}", h)))
            }),
        );
        env.define(
            "uuid".to_string(),
            Value::NativeFn(|_| {
                let a = (fast_rand_f64() * (u32::MAX as f64)) as u32;
                let b = (fast_rand_f64() * (u16::MAX as f64)) as u16;
                let c = 0x4000 | ((fast_rand_f64() * 4095.0) as u16);
                let d = 0x8000 | ((fast_rand_f64() * 16383.0) as u16);
                let e = (fast_rand_f64() * (u64::MAX as f64)) as u64 & 0xFFFF_FFFF_FFFF;
                Ok(Value::Str(format!(
                    "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
                    a, b, c, d, e
                )))
            }),
        );
        env.define(
            "clamp".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1), args.get(2)) {
                (Some(Value::Num(x)), Some(Value::Num(lo)), Some(Value::Num(hi))) => {
                    Ok(Value::Num(x.max(*lo).min(*hi)))
                }
                _ => Err("clamp(x, min, max) expects three numbers".to_string()),
            }),
        );
        env.define(
            "atan2".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Num(y)), Some(Value::Num(x))) => Ok(Value::Num(y.atan2(*x))),
                _ => Err("atan2(y, x) expects two numbers".to_string()),
            }),
        );
        env.define(
            "log".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Num(x)) => Ok(Value::Num(x.ln())),
                _ => Err("log(x) expects a number".to_string()),
            }),
        );
        env.define(
            "exp".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Num(x)) => Ok(Value::Num(x.exp())),
                _ => Err("exp(x) expects a number".to_string()),
            }),
        );
        env.define(
            "sign".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Num(x)) => Ok(Value::Num(if *x > 0.0 {
                    1.0
                } else if *x < 0.0 {
                    -1.0
                } else {
                    0.0
                })),
                _ => Err("sign(x) expects a number".to_string()),
            }),
        );
        env.define(
            "sum".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Arr(a)) => {
                    let mut total = 0.0;
                    for v in a.borrow().iter() {
                        if let Value::Num(n) = v {
                            total += *n;
                        }
                    }
                    Ok(Value::Num(total))
                }
                _ => Err("sum(array) expects an array of numbers".to_string()),
            }),
        );
        env.define(
            "avg".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Arr(a)) => {
                    let b = a.borrow();
                    if b.is_empty() {
                        return Ok(Value::Num(0.0));
                    }
                    let mut total = 0.0;
                    for v in b.iter() {
                        if let Value::Num(n) = v {
                            total += *n;
                        }
                    }
                    Ok(Value::Num(total / (b.len() as f64)))
                }
                _ => Err("avg(array) expects an array of numbers".to_string()),
            }),
        );
        env.define(
            "unique".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Arr(a)) => {
                    let mut out = Vec::new();
                    for v in a.borrow().iter() {
                        if !out.contains(v) {
                            out.push(v.clone());
                        }
                    }
                    Ok(Value::Arr(Rc::new(RefCell::new(out))))
                }
                _ => Err("unique(array) expects an array".to_string()),
            }),
        );
        env.define(
            "flat".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Arr(a)) => {
                    let mut out = Vec::new();
                    for v in a.borrow().iter() {
                        if let Value::Arr(inner) = v {
                            out.extend(inner.borrow().iter().cloned());
                        } else {
                            out.push(v.clone());
                        }
                    }
                    Ok(Value::Arr(Rc::new(RefCell::new(out))))
                }
                _ => Err("flat(array) expects an array".to_string()),
            }),
        );
        env.define(
            "enumerate".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Arr(a)) => {
                    let out: Vec<Value> = a
                        .borrow()
                        .iter()
                        .enumerate()
                        .map(|(i, v)| {
                            Value::Arr(Rc::new(RefCell::new(vec![
                                Value::Num(i as f64),
                                v.clone(),
                            ])))
                        })
                        .collect();
                    Ok(Value::Arr(Rc::new(RefCell::new(out))))
                }
                _ => Err("enumerate(array) expects an array".to_string()),
            }),
        );
        env.define(
            "zip".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Arr(a)), Some(Value::Arr(b))) => {
                    let out: Vec<Value> = a
                        .borrow()
                        .iter()
                        .zip(b.borrow().iter())
                        .map(|(x, y)| {
                            Value::Arr(Rc::new(RefCell::new(vec![x.clone(), y.clone()])))
                        })
                        .collect();
                    Ok(Value::Arr(Rc::new(RefCell::new(out))))
                }
                _ => Err("zip(arr1, arr2) expects two arrays".to_string()),
            }),
        );
        env.define(
            "index_of".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Str(sub))) => {
                    if let Some(byte_idx) = s.find(sub.as_str()) {
                        let char_idx = s[..byte_idx].chars().count();
                        Ok(Value::Num(char_idx as f64))
                    } else {
                        Ok(Value::Num(-1.0))
                    }
                }
                (Some(Value::Arr(a)), Some(needle)) => {
                    let b = a.borrow();
                    for (i, v) in b.iter().enumerate() {
                        if v == needle {
                            return Ok(Value::Num(i as f64));
                        }
                    }
                    Ok(Value::Num(-1.0))
                }
                _ => Ok(Value::Num(-1.0)),
            }),
        );
        env.define(
            "repeat".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Num(n))) => {
                    let count = (*n as isize).max(0) as usize;
                    Ok(Value::Str(s.repeat(count)))
                }
                _ => Err("repeat(str, count) expects a string and a number".to_string()),
            }),
        );
        env.define(
            "pad_start".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Num(w))) => {
                    let width = (*w as isize).max(0) as usize;
                    let pad = args
                        .get(2)
                        .and_then(|v| if let Value::Str(p) = v { Some(p.clone()) } else { None })
                        .unwrap_or_else(|| " ".to_string());
                    let cur = s.chars().count();
                    if cur >= width || pad.is_empty() {
                        return Ok(Value::Str(s.clone()));
                    }
                    let need = width - cur;
                    let mut prefix = String::new();
                    let pad_chars: Vec<char> = pad.chars().collect();
                    for i in 0..need {
                        prefix.push(pad_chars[i % pad_chars.len()]);
                    }
                    prefix.push_str(s);
                    Ok(Value::Str(prefix))
                }
                _ => Err("pad_start(str, width, [pad]) expects a string and width".to_string()),
            }),
        );
        env.define(
            "pad_end".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Num(w))) => {
                    let width = (*w as isize).max(0) as usize;
                    let pad = args
                        .get(2)
                        .and_then(|v| if let Value::Str(p) = v { Some(p.clone()) } else { None })
                        .unwrap_or_else(|| " ".to_string());
                    let cur = s.chars().count();
                    if cur >= width || pad.is_empty() {
                        return Ok(Value::Str(s.clone()));
                    }
                    let need = width - cur;
                    let mut out = s.clone();
                    let pad_chars: Vec<char> = pad.chars().collect();
                    for i in 0..need {
                        out.push(pad_chars[i % pad_chars.len()]);
                    }
                    Ok(Value::Str(out))
                }
                _ => Err("pad_end(str, width, [pad]) expects a string and width".to_string()),
            }),
        );
        env.define(
            "char_code".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Str(s)) => Ok(Value::Num(
                    s.chars().next().map(|c| c as u32 as f64).unwrap_or(0.0),
                )),
                _ => Ok(Value::Num(0.0)),
            }),
        );
        env.define(
            "from_char_code".to_string(),
            Value::NativeFn(|args| match args.first() {
                Some(Value::Num(n)) => {
                    let c = char::from_u32((*n as i64).max(0) as u32).unwrap_or('\0');
                    Ok(Value::Str(c.to_string()))
                }
                _ => Ok(Value::Str(String::new())),
            }),
        );
        env.define(
            "has_key".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Map(m)), Some(Value::Str(k))) => {
                    Ok(Value::Bool(m.borrow().contains_key(k)))
                }
                _ => Ok(Value::Bool(false)),
            }),
        );
        env.define(
            "delete_key".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Map(m)), Some(Value::Str(k))) => {
                    Ok(Value::Bool(m.borrow_mut().remove(k).is_some()))
                }
                _ => Ok(Value::Bool(false)),
            }),
        );
        env.define(
            "concat".to_string(),
            Value::NativeFn(|args| match (args.first(), args.get(1)) {
                (Some(Value::Arr(a)), Some(Value::Arr(b))) => {
                    let mut out = a.borrow().clone();
                    out.extend(b.borrow().iter().cloned());
                    Ok(Value::Arr(Rc::new(RefCell::new(out))))
                }
                _ => Err("concat(arr1, arr2) expects two arrays".to_string()),
            }),
        );

        Self {
            global: Rc::new(RefCell::new(env)),
        }
    }

    pub fn run(&mut self, program: &Program) -> Result<Value, String> {
        let mut last = Value::Null;
        let global = self.global.clone();
        for stmt in &program.statements {
            if let Stmt::Expr(e) = stmt {
                last = self.eval_expr(e, &global)?;
            } else {
                match self.exec_stmt(stmt, &global)? {
                    Signal::Return(v) => return Ok(v),
                    _ => {}
                }
            }
        }
        Ok(last)
    }

    fn exec_block(&mut self, stmts: &[Stmt], env: &Rc<RefCell<Env>>) -> Result<Signal, String> {
        let block_env = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
        let mut deferred: Vec<Stmt> = Vec::new();
        let mut result = Ok(Signal::None);
        for st in stmts {
            if let Stmt::Defer(d) = st {
                deferred.push((**d).clone());
                continue;
            }
            match self.exec_stmt(st, &block_env) {
                Ok(Signal::None) => {}
                other => {
                    result = other;
                    break;
                }
            }
        }
        for d in deferred.into_iter().rev() {
            let _ = self.exec_stmt(&d, &block_env);
        }
        result
    }

    fn exec_stmt(&mut self, stmt: &Stmt, env: &Rc<RefCell<Env>>) -> Result<Signal, String> {
        match stmt {
            Stmt::Import(path) => {
                let src = fs::read_to_string(path)
                    .map_err(|e| format!("Failed to import '{}': {}", path, e))?;
                let mut lexer = crate::lexer::Lexer::new(&src);
                let tokens = lexer.tokenize()?;
                let mut parser = crate::parser::Parser::new(tokens);
                let prog = parser.parse_program()?;
                for st in &prog.statements {
                    self.exec_stmt(st, env)?;
                }
                Ok(Signal::None)
            }
            Stmt::Let { name, value, .. } => {
                let v = self.eval_expr(value, env)?;
                env.borrow_mut().define(name.clone(), v);
                Ok(Signal::None)
            }
            Stmt::Assign { target, value } => {
                let v = self.eval_expr(value, env)?;
                match target {
                    AssignTarget::Ident(name) => {
                        env.borrow_mut().assign(name, v)?;
                    }
                    AssignTarget::Index { target, index } => {
                        let t = self.eval_expr(target, env)?;
                        let idx = self.eval_expr(index, env)?;
                        match (t, idx) {
                            (Value::Arr(a), Value::Num(n)) => {
                                let mut b = a.borrow_mut();
                                let mut i = n as isize;
                                if i < 0 {
                                    i += b.len() as isize;
                                }
                                if i >= 0 && (i as usize) < b.len() {
                                    b[i as usize] = v;
                                } else {
                                    return Err(format!("Index {} out of bounds", n));
                                }
                            }
                            (Value::Map(m), Value::Str(k)) => {
                                m.borrow_mut().insert(k, v);
                            }
                            _ => return Err("Invalid index assignment target".to_string()),
                        }
                    }
                    AssignTarget::Member { target, field } => {
                        let t = self.eval_expr(target, env)?;
                        if let Value::Map(m) = t {
                            m.borrow_mut().insert(field.clone(), v);
                        } else {
                            return Err("Member assignment requires a map or class instance".to_string());
                        }
                    }
                }
                Ok(Signal::None)
            }
            Stmt::FnDecl { name, params, body } => {
                let func = Value::Func {
                    params: params.clone(),
                    body: body.clone(),
                    closure: env.clone(),
                };
                env.borrow_mut().define(name.clone(), func);
                Ok(Signal::None)
            }
            Stmt::ClassDecl {
                name,
                parent,
                methods,
            } => {
                let cls = Value::Class {
                    name: name.clone(),
                    parent: parent.clone(),
                    methods: methods.clone(),
                    closure: env.clone(),
                };
                env.borrow_mut().define(name.clone(), cls);
                Ok(Signal::None)
            }
            Stmt::TryCatch {
                try_body,
                err_var,
                catch_body,
            } => match self.exec_block(try_body, env) {
                Ok(sig) => Ok(sig),
                Err(err_msg) => {
                    let catch_env = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
                    catch_env
                        .borrow_mut()
                        .define(err_var.clone(), Value::Str(err_msg));
                    self.exec_block(catch_body, &catch_env)
                }
            },
            Stmt::Throw(expr) => {
                let v = self.eval_expr(expr, env)?;
                Err(v.to_string())
            }
            Stmt::Test { body, .. } => self.exec_block(body, env),
            Stmt::Defer(inner) => self.exec_stmt(inner, env),
            Stmt::Return(expr) => {
                let v = match expr {
                    Some(e) => self.eval_expr(e, env)?,
                    None => Value::Null,
                };
                Ok(Signal::Return(v))
            }
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                if self.eval_expr(cond, env)?.is_truthy() {
                    self.exec_block(then_branch, env)
                } else if let Some(eb) = else_branch {
                    self.exec_block(eb, env)
                } else {
                    Ok(Signal::None)
                }
            }
            Stmt::While { cond, body } => {
                while self.eval_expr(cond, env)?.is_truthy() {
                    match self.exec_block(body, env)? {
                        Signal::None | Signal::Continue => {}
                        Signal::Break => break,
                        Signal::Return(v) => return Ok(Signal::Return(v)),
                    }
                }
                Ok(Signal::None)
            }
            Stmt::For {
                var,
                iterable,
                body,
            } => {
                let it_val = self.eval_expr(iterable, env)?;
                let items = match it_val {
                    Value::Arr(a) => a.borrow().clone(),
                    Value::Str(s) => s.chars().map(|c| Value::Str(c.to_string())).collect(),
                    Value::Map(m) => m
                        .borrow()
                        .keys()
                        .filter(|k| !k.starts_with("__"))
                        .map(|k| Value::Str(k.clone()))
                        .collect(),
                    _ => return Err("for-in expects an array, range, map, or string".to_string()),
                };
                for item in items {
                    let loop_env = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
                    loop_env.borrow_mut().define(var.clone(), item);
                    for st in body {
                        match self.exec_stmt(st, &loop_env)? {
                            Signal::None => {}
                            Signal::Continue => break,
                            Signal::Break => return Ok(Signal::None),
                            Signal::Return(v) => return Ok(Signal::Return(v)),
                        }
                    }
                }
                Ok(Signal::None)
            }
            Stmt::Break => Ok(Signal::Break),
            Stmt::Continue => Ok(Signal::Continue),
            Stmt::Expr(e) => {
                self.eval_expr(e, env)?;
                Ok(Signal::None)
            }
        }
    }

    fn instantiate_class(
        &mut self,
        class_val: Value,
        args: Vec<Value>,
        env: &Rc<RefCell<Env>>,
    ) -> Result<Value, String> {
        if let Value::Class {
            name,
            parent,
            methods,
            closure,
        } = class_val
        {
            let mut fields = BTreeMap::new();
            fields.insert("__class".to_string(), Value::Str(name.clone()));

            // Inherit ancestor methods from root down to direct parent so closer overrides win
            let mut chain = Vec::new();
            let mut cur_parent = parent.clone();
            while let Some(pname) = cur_parent {
                if let Some(Value::Class {
                    parent: next_parent,
                    methods: parent_methods,
                    closure: parent_closure,
                    ..
                }) = env.borrow().get(&pname)
                {
                    chain.push((parent_methods, parent_closure));
                    cur_parent = next_parent;
                } else {
                    break;
                }
            }
            for (parent_methods, parent_closure) in chain.into_iter().rev() {
                for m in parent_methods {
                    fields.insert(
                        m.name.clone(),
                        Value::Func {
                            params: m.params,
                            body: m.body,
                            closure: parent_closure.clone(),
                        },
                    );
                }
            }

            for m in &methods {
                fields.insert(
                    m.name.clone(),
                    Value::Func {
                        params: m.params.clone(),
                        body: m.body.clone(),
                        closure: closure.clone(),
                    },
                );
            }

            let instance = Value::Map(Rc::new(RefCell::new(fields)));
            // Run init() / constructor() if defined
            let init_fn = if let Value::Map(m) = &instance {
                let b = m.borrow();
                b.get("init").or_else(|| b.get("constructor")).cloned()
            } else {
                None
            };
            if let Some(Value::Func {
                params,
                body,
                closure: fn_closure,
            }) = init_fn
            {
                self.call_user_fn(&params, &body, &fn_closure, Some(instance.clone()), args)?;
            }
            Ok(instance)
        } else {
            Err("Cannot instantiate non-class".to_string())
        }
    }

    fn call_user_fn(
        &mut self,
        params: &[(String, Option<Expr>)],
        body: &[Stmt],
        closure: &Rc<RefCell<Env>>,
        this_val: Option<Value>,
        eval_args: Vec<Value>,
    ) -> Result<Value, String> {
        let call_env = Rc::new(RefCell::new(Env::new(Some(closure.clone()))));
        if let Some(this_obj) = this_val {
            call_env.borrow_mut().define("this".to_string(), this_obj.clone());
            call_env.borrow_mut().define("self".to_string(), this_obj);
        }
        for (i, (p_name, default_expr)) in params.iter().enumerate() {
            let val = match eval_args.get(i) {
                Some(v) => v.clone(),
                None => match default_expr {
                    Some(def) => self.eval_expr(def, &call_env)?,
                    None => Value::Null,
                },
            };
            call_env.borrow_mut().define(p_name.clone(), val);
        }
        let mut deferred: Vec<Stmt> = Vec::new();
        let mut ret_val = Value::Null;
        for st in body {
            if let Stmt::Defer(d) = st {
                deferred.push((**d).clone());
                continue;
            }
            if let Signal::Return(v) = self.exec_stmt(st, &call_env)? {
                ret_val = v;
                break;
            }
        }
        for d in deferred.into_iter().rev() {
            let _ = self.exec_stmt(&d, &call_env);
        }
        Ok(ret_val)
    }

    fn call_value(&mut self, func: Value, eval_args: Vec<Value>, env: &Rc<RefCell<Env>>) -> Result<Value, String> {
        match func {
            Value::NativeFn(f) => f(eval_args),
            Value::Func {
                params,
                body,
                closure,
            } => self.call_user_fn(&params, &body, &closure, None, eval_args),
            Value::BoundMethod {
                receiver,
                params,
                body,
                closure,
            } => self.call_user_fn(&params, &body, &closure, Some(*receiver), eval_args),
            Value::Class { .. } => self.instantiate_class(func, eval_args, env),
            other => Err(format!("Cannot call non-function ({})", other.type_name())),
        }
    }

    fn eval_expr(&mut self, expr: &Expr, env: &Rc<RefCell<Env>>) -> Result<Value, String> {
        match expr {
            Expr::Number(n) => Ok(Value::Num(*n)),
            Expr::String(s) => Ok(Value::Str(s.clone())),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Null => Ok(Value::Null),
            Expr::Ident(name) => env
                .borrow()
                .get(name)
                .ok_or_else(|| format!("Undefined variable '{}'", name)),
            Expr::Array(items) => {
                let mut vals = Vec::new();
                for it in items {
                    if let Expr::Spread(inner) = it {
                        let v = self.eval_expr(inner, env)?;
                        if let Value::Arr(a) = v {
                            vals.extend(a.borrow().iter().cloned());
                        } else {
                            vals.push(v);
                        }
                    } else {
                        vals.push(self.eval_expr(it, env)?);
                    }
                }
                Ok(Value::Arr(Rc::new(RefCell::new(vals))))
            }
            Expr::Spread(inner) => self.eval_expr(inner, env),
            Expr::Map(entries) => {
                let mut map = BTreeMap::new();
                for (k, v) in entries {
                    map.insert(k.clone(), self.eval_expr(v, env)?);
                }
                Ok(Value::Map(Rc::new(RefCell::new(map))))
            }
            Expr::New { class_name, args } => {
                let cls = env
                    .borrow()
                    .get(class_name)
                    .ok_or_else(|| format!("Undefined class '{}'", class_name))?;
                let mut eval_args = Vec::new();
                for a in args {
                    eval_args.push(self.eval_expr(a, env)?);
                }
                self.instantiate_class(cls, eval_args, env)
            }
            Expr::Ternary {
                cond,
                then_expr,
                else_expr,
            } => {
                if self.eval_expr(cond, env)?.is_truthy() {
                    self.eval_expr(then_expr, env)
                } else {
                    self.eval_expr(else_expr, env)
                }
            }
            Expr::Range { start, end } => {
                let s = self.eval_expr(start, env)?;
                let e = self.eval_expr(end, env)?;
                match (s, e) {
                    (Value::Num(a), Value::Num(b)) => {
                        let v: Vec<Value> = ((a as i64)..(b as i64))
                            .map(|i| Value::Num(i as f64))
                            .collect();
                        Ok(Value::Arr(Rc::new(RefCell::new(v))))
                    }
                    _ => Err("Range bounds '..' must be numbers".to_string()),
                }
            }
            Expr::Unary { op, expr } => {
                let v = self.eval_expr(expr, env)?;
                match op {
                    UnaryOp::Neg => match v {
                        Value::Num(n) => Ok(Value::Num(-n)),
                        _ => Err("Unary '-' requires a number".to_string()),
                    },
                    UnaryOp::Not => Ok(Value::Bool(!v.is_truthy())),
                    UnaryOp::BitNot => match v {
                        Value::Num(n) => Ok(Value::Num((!(n as i64)) as f64)),
                        _ => Err("Unary '~' requires a number".to_string()),
                    },
                }
            }
            Expr::Binary { left, op, right } => {
                if matches!(op, BinaryOp::And) {
                    let l = self.eval_expr(left, env)?;
                    if !l.is_truthy() {
                        return Ok(Value::Bool(false));
                    }
                    let r = self.eval_expr(right, env)?;
                    return Ok(Value::Bool(r.is_truthy()));
                }
                if matches!(op, BinaryOp::Or) {
                    let l = self.eval_expr(left, env)?;
                    if l.is_truthy() {
                        return Ok(Value::Bool(true));
                    }
                    let r = self.eval_expr(right, env)?;
                    return Ok(Value::Bool(r.is_truthy()));
                }
                if matches!(op, BinaryOp::NullCoalesce) {
                    let l = self.eval_expr(left, env)?;
                    if !matches!(l, Value::Null) {
                        return Ok(l);
                    }
                    return self.eval_expr(right, env);
                }

                let l = self.eval_expr(left, env)?;
                let r = self.eval_expr(right, env)?;
                match op {
                    BinaryOp::Add => match (&l, &r) {
                        (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a + b)),
                        (Value::Str(_), _) | (_, Value::Str(_)) => {
                            Ok(Value::Str(format!("{}{}", l, r)))
                        }
                        (Value::Arr(a), Value::Arr(b)) => {
                            let mut combined = a.borrow().clone();
                            combined.extend(b.borrow().iter().cloned());
                            Ok(Value::Arr(Rc::new(RefCell::new(combined))))
                        }
                        _ => Err(format!("Cannot add {} and {}", l.type_name(), r.type_name())),
                    },
                    BinaryOp::Sub => match (l, r) {
                        (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a - b)),
                        _ => Err("'-' requires numbers".to_string()),
                    },
                    BinaryOp::Mul => match (l, r) {
                        (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a * b)),
                        (Value::Str(s), Value::Num(n)) => {
                            Ok(Value::Str(s.repeat(n.max(0.0) as usize)))
                        }
                        _ => Err("'*' requires numbers".to_string()),
                    },
                    BinaryOp::Pow => match (l, r) {
                        (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a.powf(b))),
                        _ => Err("'**' requires numbers".to_string()),
                    },
                    BinaryOp::Div => match (l, r) {
                        (Value::Num(_), Value::Num(b)) if b == 0.0 => {
                            Err("Division by zero".to_string())
                        }
                        (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a / b)),
                        _ => Err("'/' requires numbers".to_string()),
                    },
                    BinaryOp::Mod => match (l, r) {
                        (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a % b)),
                        _ => Err("'%' requires numbers".to_string()),
                    },
                    BinaryOp::BitAnd => match (l, r) {
                        (Value::Num(a), Value::Num(b)) => {
                            Ok(Value::Num(((a as i64) & (b as i64)) as f64))
                        }
                        _ => Err("'&' requires numbers".to_string()),
                    },
                    BinaryOp::BitOr => match (l, r) {
                        (Value::Num(a), Value::Num(b)) => {
                            Ok(Value::Num(((a as i64) | (b as i64)) as f64))
                        }
                        _ => Err("'|' requires numbers".to_string()),
                    },
                    BinaryOp::BitXor => match (l, r) {
                        (Value::Num(a), Value::Num(b)) => {
                            Ok(Value::Num(((a as i64) ^ (b as i64)) as f64))
                        }
                        _ => Err("'^' requires numbers".to_string()),
                    },
                    BinaryOp::Shl => match (l, r) {
                        (Value::Num(a), Value::Num(b)) => {
                            Ok(Value::Num(((a as i64) << (b as i64)) as f64))
                        }
                        _ => Err("'<<' requires numbers".to_string()),
                    },
                    BinaryOp::Shr => match (l, r) {
                        (Value::Num(a), Value::Num(b)) => {
                            Ok(Value::Num(((a as i64) >> (b as i64)) as f64))
                        }
                        _ => Err("'>>' requires numbers".to_string()),
                    },
                    BinaryOp::Eq => Ok(Value::Bool(l == r)),
                    BinaryOp::Neq => Ok(Value::Bool(l != r)),
                    BinaryOp::Lt => match (l, r) {
                        (Value::Num(a), Value::Num(b)) => Ok(Value::Bool(a < b)),
                        (Value::Str(a), Value::Str(b)) => Ok(Value::Bool(a < b)),
                        _ => Err("'<' requires numbers or strings".to_string()),
                    },
                    BinaryOp::Gt => match (l, r) {
                        (Value::Num(a), Value::Num(b)) => Ok(Value::Bool(a > b)),
                        (Value::Str(a), Value::Str(b)) => Ok(Value::Bool(a > b)),
                        _ => Err("'>' requires numbers or strings".to_string()),
                    },
                    BinaryOp::Lte => match (l, r) {
                        (Value::Num(a), Value::Num(b)) => Ok(Value::Bool(a <= b)),
                        (Value::Str(a), Value::Str(b)) => Ok(Value::Bool(a <= b)),
                        _ => Err("'<=' requires numbers or strings".to_string()),
                    },
                    BinaryOp::Gte => match (l, r) {
                        (Value::Num(a), Value::Num(b)) => Ok(Value::Bool(a >= b)),
                        (Value::Str(a), Value::Str(b)) => Ok(Value::Bool(a >= b)),
                        _ => Err("'>=' requires numbers or strings".to_string()),
                    },
                    _ => unreachable!(),
                }
            }
            Expr::Call { callee, args } => {
                // Support built-in higher-order functions: map(arr, fn), filter(arr, fn), reduce(arr, fn, init)
                if let Expr::Ident(name) = &**callee {
                    if name == "map" && args.len() == 2 {
                        let arr_val = self.eval_expr(&args[0], env)?;
                        let fn_val = self.eval_expr(&args[1], env)?;
                        if let Value::Arr(a) = arr_val {
                            let items = a.borrow().clone();
                            let mut out = Vec::new();
                            for it in items {
                                out.push(self.call_value(fn_val.clone(), vec![it], env)?);
                            }
                            return Ok(Value::Arr(Rc::new(RefCell::new(out))));
                        }
                    } else if name == "filter" && args.len() == 2 {
                        let arr_val = self.eval_expr(&args[0], env)?;
                        let fn_val = self.eval_expr(&args[1], env)?;
                        if let Value::Arr(a) = arr_val {
                            let items = a.borrow().clone();
                            let mut out = Vec::new();
                            for it in items {
                                if self.call_value(fn_val.clone(), vec![it.clone()], env)?.is_truthy() {
                                    out.push(it);
                                }
                            }
                            return Ok(Value::Arr(Rc::new(RefCell::new(out))));
                        }
                    } else if name == "reduce" && args.len() >= 2 {
                        let arr_val = self.eval_expr(&args[0], env)?;
                        let fn_val = self.eval_expr(&args[1], env)?;
                        if let Value::Arr(a) = arr_val {
                            let items = a.borrow().clone();
                            let mut iter = items.into_iter();
                            let mut acc = if let Some(init_expr) = args.get(2) {
                                self.eval_expr(init_expr, env)?
                            } else {
                                iter.next().unwrap_or(Value::Null)
                            };
                            for it in iter {
                                acc = self.call_value(fn_val.clone(), vec![acc, it], env)?;
                            }
                            return Ok(acc);
                        }
                    } else if name == "find" && args.len() == 2 {
                        let arr_val = self.eval_expr(&args[0], env)?;
                        let fn_val = self.eval_expr(&args[1], env)?;
                        if let Value::Arr(a) = arr_val {
                            let items = a.borrow().clone();
                            for it in items {
                                if self.call_value(fn_val.clone(), vec![it.clone()], env)?.is_truthy() {
                                    return Ok(it);
                                }
                            }
                            return Ok(Value::Null);
                        }
                    } else if name == "any" && args.len() == 2 {
                        let arr_val = self.eval_expr(&args[0], env)?;
                        let fn_val = self.eval_expr(&args[1], env)?;
                        if let Value::Arr(a) = arr_val {
                            let items = a.borrow().clone();
                            for it in items {
                                if self.call_value(fn_val.clone(), vec![it], env)?.is_truthy() {
                                    return Ok(Value::Bool(true));
                                }
                            }
                            return Ok(Value::Bool(false));
                        }
                    } else if name == "all" && args.len() == 2 {
                        let arr_val = self.eval_expr(&args[0], env)?;
                        let fn_val = self.eval_expr(&args[1], env)?;
                        if let Value::Arr(a) = arr_val {
                            let items = a.borrow().clone();
                            for it in items {
                                if !self.call_value(fn_val.clone(), vec![it], env)?.is_truthy() {
                                    return Ok(Value::Bool(false));
                                }
                            }
                            return Ok(Value::Bool(true));
                        }
                    }
                }

                let func = self.eval_expr(callee, env)?;
                let mut eval_args = Vec::new();
                for a in args {
                    eval_args.push(self.eval_expr(a, env)?);
                }
                self.call_value(func, eval_args, env)
            }
            Expr::Index { target, index } => {
                let t = self.eval_expr(target, env)?;
                let idx = self.eval_expr(index, env)?;
                match (t, idx) {
                    (Value::Arr(a), Value::Num(n)) => {
                        let b = a.borrow();
                        let i = if n < 0.0 {
                            (b.len() as isize + n as isize) as usize
                        } else {
                            n as usize
                        };
                        Ok(b.get(i).cloned().unwrap_or(Value::Null))
                    }
                    (Value::Map(m), Value::Str(k)) => {
                        Ok(m.borrow().get(&k).cloned().unwrap_or(Value::Null))
                    }
                    (Value::Str(s), Value::Num(n)) => Ok(s
                        .chars()
                        .nth(n as usize)
                        .map(|c| Value::Str(c.to_string()))
                        .unwrap_or(Value::Null)),
                    _ => Ok(Value::Null),
                }
            }
            Expr::Member { target, field } => {
                let t = self.eval_expr(target, env)?;
                match &t {
                    Value::Map(m) => {
                        let val = m.borrow().get(field).cloned().unwrap_or(Value::Null);
                        if let Value::Func {
                            params,
                            body,
                            closure,
                        } = val
                        {
                            Ok(Value::BoundMethod {
                                receiver: Box::new(t),
                                params,
                                body,
                                closure,
                            })
                        } else {
                            Ok(val)
                        }
                    }
                    Value::Arr(a) if field == "length" || field == "len" => {
                        Ok(Value::Num(a.borrow().len() as f64))
                    }
                    Value::Str(s) if field == "length" || field == "len" => {
                        Ok(Value::Num(s.chars().count() as f64))
                    }
                    _ => Ok(Value::Null),
                }
            }
            Expr::FnExpr { params, body } => Ok(Value::Func {
                params: params.clone(),
                body: body.clone(),
                closure: env.clone(),
            }),
        }
    }
}
