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
