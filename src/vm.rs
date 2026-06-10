use crate::ast::{AssignTarget, BinaryOp, Expr, Program, Stmt, UnaryOp};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone)]
pub enum Op {
    LoadConstNum(f64),
    LoadConstStr(String),
    LoadConstBool(bool),
    LoadNull,
    LoadVar(String),
    StoreVar(String),
    Add,
    Sub,
    Mul,
    Pow,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    Neg,
    Not,
    MakeArray(usize),
    MakeRange,
    GetIndex,
    Call(usize),
    Print(usize),
    Jump(usize),
