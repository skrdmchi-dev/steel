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
            out.push_str(&format!("  {:04}  {}\n", idx, op));
        }
        out
    }

    /// Executes the compiled Stack VM Bytecode directly!
    pub fn execute(&self, quiet: bool) -> Result<String, String> {
        #[derive(Clone, Debug)]
        enum VmVal {
            Null,
            Bool(bool),
            Num(f64),
            Str(String),
            Arr(Vec<VmVal>),
        }
        impl VmVal {
            fn as_num(&self) -> f64 {
                match self {
                    VmVal::Num(n) => *n,
                    VmVal::Bool(true) => 1.0,
                    _ => 0.0,
                }
            }
            fn truthy(&self) -> bool {
                match self {
                    VmVal::Null => false,
                    VmVal::Bool(b) => *b,
                    VmVal::Num(n) => *n != 0.0,
                    VmVal::Str(s) => !s.is_empty(),
                    VmVal::Arr(a) => !a.is_empty(),
                }
            }
            fn display(&self) -> String {
                match self {
                    VmVal::Null => "null".to_string(),
                    VmVal::Bool(b) => b.to_string(),
                    VmVal::Num(n) => {
                        if n.fract() == 0.0 {
                            format!("{:.0}", n)
                        } else {
                            format!("{}", n)
                        }
                    }
                    VmVal::Str(s) => s.clone(),
                    VmVal::Arr(a) => {
                        let inner: Vec<String> = a.iter().map(|x| x.display()).collect();
                        format!("[{}]", inner.join(", "))
                    }
                }
            }
        }

        fn run_ops(
            ops: &[Op],
            vars: &mut BTreeMap<String, VmVal>,
            fns: &BTreeMap<String, (Vec<String>, Vec<Op>)>,
            quiet: bool,
        ) -> Result<VmVal, String> {
            let mut stack: Vec<VmVal> = Vec::with_capacity(64);
            let mut pc = 0;
            while pc < ops.len() {
                match &ops[pc] {
                    Op::LoadConstNum(n) => stack.push(VmVal::Num(*n)),
                    Op::LoadConstStr(s) => stack.push(VmVal::Str(s.clone())),
                    Op::LoadConstBool(b) => stack.push(VmVal::Bool(*b)),
                    Op::LoadNull => stack.push(VmVal::Null),
                    Op::LoadVar(name) => {
                        if let Some(v) = vars.get(name) {
                            stack.push(v.clone());
                        } else {
                            stack.push(VmVal::Str(name.clone()));
                        }
                    }
                    Op::StoreVar(name) => {
                        let val = stack.pop().unwrap_or(VmVal::Null);
                        vars.insert(name.clone(), val);
                    }
                    Op::Add => {
                        let r = stack.pop().unwrap_or(VmVal::Null);
                        let l = stack.pop().unwrap_or(VmVal::Null);
                        match (&l, &r) {
                            (VmVal::Str(_), _) | (_, VmVal::Str(_)) => {
                                stack.push(VmVal::Str(format!("{}{}", l.display(), r.display())));
                            }
                            _ => stack.push(VmVal::Num(l.as_num() + r.as_num())),
                        }
                    }
                    Op::Sub => {
                        let r = stack.pop().unwrap_or(VmVal::Null);
                        let l = stack.pop().unwrap_or(VmVal::Null);
                        stack.push(VmVal::Num(l.as_num() - r.as_num()));
                    }
                    Op::Mul => {
                        let r = stack.pop().unwrap_or(VmVal::Null);
                        let l = stack.pop().unwrap_or(VmVal::Null);
                        stack.push(VmVal::Num(l.as_num() * r.as_num()));
                    }
                    Op::Pow => {
                        let r = stack.pop().unwrap_or(VmVal::Null);
                        let l = stack.pop().unwrap_or(VmVal::Null);
                        stack.push(VmVal::Num(l.as_num().powf(r.as_num())));
                    }
                    Op::Div => {
                        let r = stack.pop().unwrap_or(VmVal::Null);
                        let l = stack.pop().unwrap_or(VmVal::Null);
                        stack.push(VmVal::Num(l.as_num() / r.as_num()));
                    }
                    Op::Mod => {
                        let r = stack.pop().unwrap_or(VmVal::Null);
                        let l = stack.pop().unwrap_or(VmVal::Null);
                        stack.push(VmVal::Num(l.as_num() % r.as_num()));
                    }
                    Op::Eq => {
                        let r = stack.pop().unwrap_or(VmVal::Null);
                        let l = stack.pop().unwrap_or(VmVal::Null);
                        stack.push(VmVal::Bool(l.display() == r.display()));
                    }
                    Op::Neq => {
                        let r = stack.pop().unwrap_or(VmVal::Null);
                        let l = stack.pop().unwrap_or(VmVal::Null);
                        stack.push(VmVal::Bool(l.display() != r.display()));
                    }
                    Op::Lt => {
                        let r = stack.pop().unwrap_or(VmVal::Null);
                        let l = stack.pop().unwrap_or(VmVal::Null);
                        stack.push(VmVal::Bool(l.as_num() < r.as_num()));
                    }
                    Op::Gt => {
