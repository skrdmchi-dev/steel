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
            name,
            is_const,
            value,
        } => {
            let kw = if *is_const { "const" } else { "let" };
            format!("{}{} {} = {}\n", p, kw, name, fmt_expr(value))
        }
        Stmt::Assign { target, value } => {
            let lhs = match target {
                AssignTarget::Ident(n) => n.clone(),
                AssignTarget::Index { target, index } => {
                    format!("{}[{}]", fmt_expr(target), fmt_expr(index))
                }
                AssignTarget::Member { target, field } => {
                    format!("{}.{}", fmt_expr(target), field)
                }
            };
            format!("{}{} = {}\n", p, lhs, fmt_expr(value))
        }
        Stmt::FnDecl { name, params, body } => {
            let mut s = format!("{}fn {}({}) {{\n", p, name, fmt_params(params));
            for st in body {
                s.push_str(&fmt_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}}}\n", p));
            s
        }
        Stmt::ClassDecl {
            name,
            parent,
            methods,
        } => {
            let ext = parent
                .as_ref()
                .map(|b| format!(" extends {}", b))
                .unwrap_or_default();
            let mut s = format!("{}class {}{} {{\n", p, name, ext);
            for m in methods {
                s.push_str(&format!(
                    "{}    {}({}) {{\n",
                    p,
                    m.name,
                    fmt_params(&m.params)
                ));
                for st in &m.body {
                    s.push_str(&fmt_stmt(st, lvl + 2));
                }
                s.push_str(&format!("{}    }}\n", p));
            }
            s.push_str(&format!("{}}}\n", p));
            s
        }
        Stmt::TryCatch {
            try_body,
            err_var,
            catch_body,
        } => {
            let mut s = format!("{}try {{\n", p);
            for st in try_body {
                s.push_str(&fmt_stmt(st, lvl + 1));
            }
