use crate::ast::{AssignTarget, BinaryOp, Expr, Program, Stmt, UnaryOp};

pub fn format_program(program: &Program) -> String {
    let mut out = String::new();
    for stmt in &program.statements {
        out.push_str(&fmt_stmt(stmt, 0));
    }
    out
}

fn pad(lvl: usize) -> String {
    "    ".repeat(lvl)
}

fn fmt_params(params: &[(String, Option<Expr>)]) -> String {
    params
        .iter()
        .map(|(n, def)| match def {
            Some(e) => format!("{} = {}", n, fmt_expr(e)),
            None => n.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn fmt_stmt(stmt: &Stmt, lvl: usize) -> String {
    let p = pad(lvl);
    match stmt {
        Stmt::Import(path) => format!("{}import \"{}\"\n", p, path),
        Stmt::Let {
