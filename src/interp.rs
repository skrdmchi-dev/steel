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
