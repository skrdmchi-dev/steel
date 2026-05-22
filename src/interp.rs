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
