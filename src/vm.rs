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
    JumpIfFalse(usize),
    Pop,
    Return,
    Halt,
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Op::LoadConstNum(n) => write!(f, "LOAD_CONST_NUM   {}", n),
            Op::LoadConstStr(s) => write!(f, "LOAD_CONST_STR   {:?}", s),
            Op::LoadConstBool(b) => write!(f, "LOAD_CONST_BOOL  {}", b),
            Op::LoadNull => write!(f, "LOAD_NULL"),
            Op::LoadVar(name) => write!(f, "LOAD_VAR         {}", name),
            Op::StoreVar(name) => write!(f, "STORE_VAR        {}", name),
            Op::Add => write!(f, "BINARY_ADD"),
            Op::Sub => write!(f, "BINARY_SUB"),
            Op::Mul => write!(f, "BINARY_MUL"),
            Op::Pow => write!(f, "BINARY_POW"),
            Op::Div => write!(f, "BINARY_DIV"),
            Op::Mod => write!(f, "BINARY_MOD"),
            Op::Eq => write!(f, "COMPARE_EQ"),
            Op::Neq => write!(f, "COMPARE_NEQ"),
            Op::Lt => write!(f, "COMPARE_LT"),
            Op::Gt => write!(f, "COMPARE_GT"),
            Op::Lte => write!(f, "COMPARE_LTE"),
            Op::Gte => write!(f, "COMPARE_GTE"),
            Op::Neg => write!(f, "UNARY_NEG"),
            Op::Not => write!(f, "UNARY_NOT"),
            Op::MakeArray(n) => write!(f, "MAKE_ARRAY       {}", n),
            Op::MakeRange => write!(f, "MAKE_RANGE"),
            Op::GetIndex => write!(f, "GET_INDEX"),
            Op::Call(argc) => write!(f, "CALL_FN          argc={}", argc),
            Op::Print(argc) => write!(f, "BUILTIN_PRINT    argc={}", argc),
            Op::Jump(addr) => write!(f, "JUMP             -> {:04}", addr),
            Op::JumpIfFalse(addr) => write!(f, "JUMP_IF_FALSE    -> {:04}", addr),
            Op::Pop => write!(f, "POP_TOP"),
            Op::Return => write!(f, "RETURN_VALUE"),
            Op::Halt => write!(f, "HALT"),
        }
    }
}

pub struct BytecodeChunk {
    pub ops: Vec<Op>,
    pub functions: BTreeMap<String, (Vec<String>, Vec<Op>)>,
}

impl BytecodeChunk {
    pub fn disassemble(&self) -> String {
        let mut out = String::new();
        for (fn_name, (params, fn_ops)) in &self.functions {
            out.push_str(&format!(
                "=== Function: {}({}) ===\n",
                fn_name,
                params.join(", ")
            ));
            for (idx, op) in fn_ops.iter().enumerate() {
                out.push_str(&format!("  {:04}  {}\n", idx, op));
            }
            out.push('\n');
        }
        out.push_str("=== Main Entry Chunk ===\n");
        for (idx, op) in self.ops.iter().enumerate() {
