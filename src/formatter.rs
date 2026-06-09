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
            s.push_str(&format!("{}}} catch ({}) {{\n", p, err_var));
            for st in catch_body {
                s.push_str(&fmt_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}}}\n", p));
            s
        }
        Stmt::Throw(e) => format!("{}throw {}\n", p, fmt_expr(e)),
        Stmt::Test { name, body } => {
            let mut s = format!("{}test \"{}\" {{\n", p, name);
            for st in body {
                s.push_str(&fmt_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}}}\n", p));
            s
        }
        Stmt::Defer(inner) => format!("{}defer {}", p, fmt_stmt(inner, 0)),
        Stmt::Return(Some(e)) => format!("{}return {}\n", p, fmt_expr(e)),
        Stmt::Return(None) => format!("{}return\n", p),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let mut s = format!("{}if {} {{\n", p, fmt_expr(cond));
            for st in then_branch {
                s.push_str(&fmt_stmt(st, lvl + 1));
            }
            if let Some(eb) = else_branch {
                s.push_str(&format!("{}}} else {{\n", p));
                for st in eb {
                    s.push_str(&fmt_stmt(st, lvl + 1));
                }
            }
            s.push_str(&format!("{}}}\n", p));
            s
        }
        Stmt::While { cond, body } => {
            let mut s = format!("{}while {} {{\n", p, fmt_expr(cond));
            for st in body {
                s.push_str(&fmt_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}}}\n", p));
            s
        }
        Stmt::For {
            var,
            iterable,
            body,
        } => {
            let mut s = format!("{}for {} in {} {{\n", p, var, fmt_expr(iterable));
            for st in body {
                s.push_str(&fmt_stmt(st, lvl + 1));
            }
            s.push_str(&format!("{}}}\n", p));
            s
        }
        Stmt::Break => format!("{}break\n", p),
        Stmt::Continue => format!("{}continue\n", p),
        Stmt::Expr(e) => format!("{}{}\n", p, fmt_expr(e)),
    }
}

fn fmt_expr(expr: &Expr) -> String {
    match expr {
        Expr::Number(n) => {
            if n.fract() == 0.0 {
                format!("{:.0}", n)
            } else {
                format!("{}", n)
            }
        }
        Expr::String(s) => format!("{:?}", s),
        Expr::Bool(b) => b.to_string(),
        Expr::Null => "null".to_string(),
        Expr::Ident(n) => n.clone(),
        Expr::Array(items) => {
            let inner: Vec<String> = items.iter().map(fmt_expr).collect();
            format!("[{}]", inner.join(", "))
        }
        Expr::Map(entries) => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{}: {}", k, fmt_expr(v)))
                .collect();
            format!("{{ {} }}", inner.join(", "))
        }
        Expr::New { class_name, args } => {
            let a: Vec<String> = args.iter().map(fmt_expr).collect();
            format!("new {}({})", class_name, a.join(", "))
        }
        Expr::Ternary {
            cond,
            then_expr,
            else_expr,
        } => format!(
            "{} ? {} : {}",
            fmt_expr(cond),
            fmt_expr(then_expr),
            fmt_expr(else_expr)
        ),
        Expr::Binary { left, op, right } => {
            let op_s = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Pow => "**",
                BinaryOp::Div => "/",
                BinaryOp::Mod => "%",
                BinaryOp::Eq => "==",
                BinaryOp::Neq => "!=",
                BinaryOp::Lt => "<",
                BinaryOp::Gt => ">",
                BinaryOp::Lte => "<=",
                BinaryOp::Gte => ">=",
                BinaryOp::And => "&&",
                BinaryOp::Or => "||",
                BinaryOp::BitAnd => "&",
                BinaryOp::BitOr => "|",
                BinaryOp::BitXor => "^",
                BinaryOp::Shl => "<<",
                BinaryOp::Shr => ">>",
                BinaryOp::NullCoalesce => "??",
            };
            format!("({} {} {})", fmt_expr(left), op_s, fmt_expr(right))
        }
        Expr::Unary { op, expr: inner } => {
            let op_s = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
                UnaryOp::BitNot => "~",
            };
            format!("{}{}", op_s, fmt_expr(inner))
        }
        Expr::Call { callee, args } => {
            let a: Vec<String> = args.iter().map(fmt_expr).collect();
            format!("{}({})", fmt_expr(callee), a.join(", "))
        }
        Expr::Index { target, index } => format!("{}[{}]", fmt_expr(target), fmt_expr(index)),
        Expr::Member { target, field } => format!("{}.{}", fmt_expr(target), field),
        Expr::FnExpr { params, body } => {
            let mut s = format!("fn({}) {{\n", fmt_params(params));
            for st in body {
                s.push_str(&fmt_stmt(st, 1));
            }
            s.push('}');
            s
        }
        Expr::Range { start, end } => format!("{}..{}", fmt_expr(start), fmt_expr(end)),
        Expr::Spread(inner) => format!("...{}", fmt_expr(inner)),
    }
}
