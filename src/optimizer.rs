use crate::ast::{AssignTarget, BinaryOp, ClassMethod, Expr, Program, Stmt, UnaryOp};
use std::collections::{HashMap, HashSet};

/// Constant-folding, dead-branch elimination, and default-argument lowering AST optimizer pass.
/// Folds constant arithmetic (`2 ** 10` -> `1024`), string concatenations (`"a" + "b"` -> `"ab"`),
/// boolean logic, constant `if true` / `if false` branches, and fills omitted default arguments.
pub fn optimize_program(program: &Program) -> Program {
    let mut defaults: HashMap<String, Vec<Option<Expr>>> = HashMap::new();
    for st in &program.statements {
        match st {
            Stmt::FnDecl { name, params, .. } => {
                if params.iter().any(|(_, d)| d.is_some()) {
                    defaults.insert(
                        name.clone(),
                        params.iter().map(|(_, d)| d.as_ref().map(opt_expr)).collect(),
                    );
                }
            }
            Stmt::ClassDecl { name, methods, .. } => {
                for m in methods {
                    if (m.name == "init" || m.name == "constructor")
                        && m.params.iter().any(|(_, d)| d.is_some())
                    {
                        defaults.insert(
                            name.clone(),
                            m.params.iter().map(|(_, d)| d.as_ref().map(opt_expr)).collect(),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    let mut stmts: Vec<Stmt> = program.statements.iter().map(opt_stmt).collect();
    flatten_class_inheritance(&mut stmts);
    if !defaults.is_empty() {
        stmts = stmts
            .into_iter()
            .map(|s| fill_defaults_stmt(s, &defaults))
            .collect();
    }
    let mut scope: HashSet<String> = HashSet::new();
    for st in &stmts {
        if let Stmt::FnDecl { name, .. } | Stmt::ClassDecl { name, .. } = st {
            scope.insert(name.clone());
        }
    }
    let stmts = promote_implicit_lets(stmts, &mut scope);
    let stmts = box_mutable_captures_block(stmts, &[]);
    Program { statements: stmts }
}

/// Promotes first-time bare assignments (`x = 10`) to `Stmt::Let` so `let` is optional
/// across Steel and all 8 compiled target languages!
fn promote_implicit_lets(stmts: Vec<Stmt>, scope: &mut HashSet<String>) -> Vec<Stmt> {
    let mut out = Vec::with_capacity(stmts.len());
    for st in stmts {
        match st {
            Stmt::Let { name, is_const, value } => {
                scope.insert(name.clone());
                out.push(Stmt::Let { name, is_const, value });
            }
            Stmt::Assign { target: AssignTarget::Ident(name), value } => {
                if !scope.contains(&name) {
                    scope.insert(name.clone());
                    out.push(Stmt::Let { name, is_const: false, value });
                } else {
                    out.push(Stmt::Assign { target: AssignTarget::Ident(name), value });
                }
            }
            Stmt::FnDecl { name, params, body } => {
                scope.insert(name.clone());
                let mut fn_scope = scope.clone();
                for (p, _) in &params {
                    fn_scope.insert(p.clone());
                }
                let new_body = promote_implicit_lets(body, &mut fn_scope);
                out.push(Stmt::FnDecl { name, params, body: new_body });
            }
            Stmt::ClassDecl { name, parent, methods } => {
                scope.insert(name.clone());
                let new_methods = methods
                    .into_iter()
                    .map(|m| {
                        let mut m_scope = scope.clone();
                        m_scope.insert("this".to_string());
                        m_scope.insert("self".to_string());
                        for (p, _) in &m.params {
                            m_scope.insert(p.clone());
                        }
                        let new_body = promote_implicit_lets(m.body, &mut m_scope);
                        ClassMethod { name: m.name, params: m.params, body: new_body }
                    })
                    .collect();
                out.push(Stmt::ClassDecl { name, parent, methods: new_methods });
            }
            Stmt::If { cond, then_branch, else_branch } => {
                let mut t_scope = scope.clone();
                let tb = promote_implicit_lets(then_branch, &mut t_scope);
                let eb = else_branch.map(|b| {
                    let mut e_scope = scope.clone();
                    promote_implicit_lets(b, &mut e_scope)
                });
                out.push(Stmt::If { cond, then_branch: tb, else_branch: eb });
            }
            Stmt::While { cond, body } => {
                let mut w_scope = scope.clone();
                let nb = promote_implicit_lets(body, &mut w_scope);
                out.push(Stmt::While { cond, body: nb });
            }
            Stmt::For { var, iterable, body } => {
                let mut f_scope = scope.clone();
                f_scope.insert(var.clone());
                let nb = promote_implicit_lets(body, &mut f_scope);
                out.push(Stmt::For { var, iterable, body: nb });
            }
            Stmt::TryCatch { try_body, err_var, catch_body } => {
                let mut ts = scope.clone();
                let tb = promote_implicit_lets(try_body, &mut ts);
                let mut cs = scope.clone();
                cs.insert(err_var.clone());
                let cb = promote_implicit_lets(catch_body, &mut cs);
                out.push(Stmt::TryCatch { try_body: tb, err_var, catch_body: cb });
            }
            Stmt::Test { name, body } => {
                let mut ts = scope.clone();
                let nb = promote_implicit_lets(body, &mut ts);
                out.push(Stmt::Test { name, body: nb });
            }
            other => out.push(other),
        }
    }
    out
}

fn fill_defaults_stmt(stmt: Stmt, defs: &HashMap<String, Vec<Option<Expr>>>) -> Stmt {
    match stmt {
        Stmt::Let { name, is_const, value } => Stmt::Let {
            name,
            is_const,
            value: fill_defaults_expr(value, defs),
        },
        Stmt::Assign { target, value } => {
            let t = match target {
                AssignTarget::Ident(n) => AssignTarget::Ident(n),
                AssignTarget::Index { target, index } => AssignTarget::Index {
                    target: fill_defaults_expr(target, defs),
                    index: fill_defaults_expr(index, defs),
                },
                AssignTarget::Member { target, field } => AssignTarget::Member {
                    target: fill_defaults_expr(target, defs),
                    field,
                },
            };
            Stmt::Assign {
                target: t,
                value: fill_defaults_expr(value, defs),
            }
        }
        Stmt::FnDecl { name, params, body } => Stmt::FnDecl {
            name,
            params,
            body: body.into_iter().map(|s| fill_defaults_stmt(s, defs)).collect(),
        },
        Stmt::ClassDecl { name, parent, methods } => Stmt::ClassDecl {
            name,
            parent,
            methods: methods
                .into_iter()
                .map(|m| ClassMethod {
                    name: m.name,
                    params: m.params,
                    body: m.body.into_iter().map(|s| fill_defaults_stmt(s, defs)).collect(),
                })
                .collect(),
        },
        Stmt::TryCatch { try_body, err_var, catch_body } => Stmt::TryCatch {
            try_body: try_body.into_iter().map(|s| fill_defaults_stmt(s, defs)).collect(),
            err_var,
            catch_body: catch_body.into_iter().map(|s| fill_defaults_stmt(s, defs)).collect(),
        },
        Stmt::Throw(e) => Stmt::Throw(fill_defaults_expr(e, defs)),
        Stmt::Test { name, body } => Stmt::Test {
            name,
            body: body.into_iter().map(|s| fill_defaults_stmt(s, defs)).collect(),
        },
        Stmt::Defer(inner) => Stmt::Defer(Box::new(fill_defaults_stmt(*inner, defs))),
        Stmt::Return(e) => Stmt::Return(e.map(|x| fill_defaults_expr(x, defs))),
        Stmt::If { cond, then_branch, else_branch } => Stmt::If {
            cond: fill_defaults_expr(cond, defs),
            then_branch: then_branch.into_iter().map(|s| fill_defaults_stmt(s, defs)).collect(),
            else_branch: else_branch.map(|eb| eb.into_iter().map(|s| fill_defaults_stmt(s, defs)).collect()),
        },
        Stmt::While { cond, body } => Stmt::While {
            cond: fill_defaults_expr(cond, defs),
            body: body.into_iter().map(|s| fill_defaults_stmt(s, defs)).collect(),
        },
        Stmt::For { var, iterable, body } => Stmt::For {
            var,
            iterable: fill_defaults_expr(iterable, defs),
            body: body.into_iter().map(|s| fill_defaults_stmt(s, defs)).collect(),
        },
        Stmt::Expr(e) => Stmt::Expr(fill_defaults_expr(e, defs)),
        other => other,
    }
}

fn fill_defaults_expr(expr: Expr, defs: &HashMap<String, Vec<Option<Expr>>>) -> Expr {
    match expr {
        Expr::Call { callee, args } => {
            let c = fill_defaults_expr(*callee, defs);
            let mut new_args: Vec<Expr> = args.into_iter().map(|a| fill_defaults_expr(a, defs)).collect();
            if let Expr::Ident(ref name) = c {
                if let Some(param_defs) = defs.get(name) {
                    while new_args.len() < param_defs.len() {
                        let idx = new_args.len();
                        let def_val = param_defs[idx].clone().unwrap_or(Expr::Null);
                        new_args.push(def_val);
                    }
                }
            }
            Expr::Call { callee: Box::new(c), args: new_args }
        }
        Expr::New { class_name, args } => {
            let mut new_args: Vec<Expr> = args.into_iter().map(|a| fill_defaults_expr(a, defs)).collect();
            if let Some(param_defs) = defs.get(&class_name) {
                while new_args.len() < param_defs.len() {
                    let idx = new_args.len();
                    let def_val = param_defs[idx].clone().unwrap_or(Expr::Null);
                    new_args.push(def_val);
                }
            }
            Expr::New { class_name, args: new_args }
        }
        Expr::Array(items) => Expr::Array(items.into_iter().map(|e| fill_defaults_expr(e, defs)).collect()),
        Expr::Map(entries) => Expr::Map(entries.into_iter().map(|(k, v)| (k, fill_defaults_expr(v, defs))).collect()),
        Expr::Binary { left, op, right } => Expr::Binary {
            left: Box::new(fill_defaults_expr(*left, defs)),
            op,
            right: Box::new(fill_defaults_expr(*right, defs)),
        },
        Expr::Unary { op, expr } => Expr::Unary { op, expr: Box::new(fill_defaults_expr(*expr, defs)) },
        Expr::Ternary { cond, then_expr, else_expr } => Expr::Ternary {
            cond: Box::new(fill_defaults_expr(*cond, defs)),
            then_expr: Box::new(fill_defaults_expr(*then_expr, defs)),
            else_expr: Box::new(fill_defaults_expr(*else_expr, defs)),
        },
        Expr::Index { target, index } => Expr::Index {
            target: Box::new(fill_defaults_expr(*target, defs)),
            index: Box::new(fill_defaults_expr(*index, defs)),
        },
        Expr::Member { target, field } => Expr::Member {
            target: Box::new(fill_defaults_expr(*target, defs)),
            field,
        },
        Expr::Range { start, end } => Expr::Range {
            start: Box::new(fill_defaults_expr(*start, defs)),
            end: Box::new(fill_defaults_expr(*end, defs)),
        },
        Expr::Spread(inner) => Expr::Spread(Box::new(fill_defaults_expr(*inner, defs))),
        Expr::FnExpr { params, body } => Expr::FnExpr {
            params,
            body: body.into_iter().map(|s| fill_defaults_stmt(s, defs)).collect(),
        },
        other => other,
    }
}

fn opt_stmt(stmt: &Stmt) -> Stmt {
    match stmt {
        Stmt::Let {
            name,
            is_const,
            value,
        } => Stmt::Let {
            name: name.clone(),
            is_const: *is_const,
            value: opt_expr(value),
        },
        Stmt::Assign { target, value } => {
            let t = match target {
                AssignTarget::Ident(n) => AssignTarget::Ident(n.clone()),
                AssignTarget::Index { target, index } => AssignTarget::Index {
                    target: opt_expr(target),
                    index: opt_expr(index),
                },
                AssignTarget::Member { target, field } => AssignTarget::Member {
                    target: opt_expr(target),
                    field: field.clone(),
                },
            };
            Stmt::Assign {
                target: t,
                value: opt_expr(value),
            }
        }
        Stmt::FnDecl { name, params, body } => Stmt::FnDecl {
            name: name.clone(),
            params: params
                .iter()
                .map(|(n, d)| (n.clone(), d.as_ref().map(opt_expr)))
                .collect(),
            body: body.iter().map(opt_stmt).collect(),
        },
        Stmt::ClassDecl {
            name,
            parent,
            methods,
        } => Stmt::ClassDecl {
            name: name.clone(),
            parent: parent.clone(),
            methods: methods
                .iter()
                .map(|m| ClassMethod {
                    name: m.name.clone(),
                    params: m
                        .params
                        .iter()
                        .map(|(n, d)| (n.clone(), d.as_ref().map(opt_expr)))
                        .collect(),
                    body: m.body.iter().map(opt_stmt).collect(),
                })
                .collect(),
        },
        Stmt::TryCatch {
            try_body,
            err_var,
            catch_body,
        } => Stmt::TryCatch {
            try_body: try_body.iter().map(opt_stmt).collect(),
            err_var: err_var.clone(),
            catch_body: catch_body.iter().map(opt_stmt).collect(),
        },
        Stmt::Throw(e) => Stmt::Throw(opt_expr(e)),
        Stmt::Test { name, body } => Stmt::Test {
            name: name.clone(),
            body: body.iter().map(opt_stmt).collect(),
        },
        Stmt::Defer(inner) => Stmt::Defer(Box::new(opt_stmt(inner))),
        Stmt::Return(e) => Stmt::Return(e.as_ref().map(opt_expr)),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => Stmt::If {
            cond: opt_expr(cond),
            then_branch: then_branch.iter().map(opt_stmt).collect(),
            else_branch: else_branch
                .as_ref()
                .map(|eb| eb.iter().map(opt_stmt).collect()),
        },
        Stmt::While { cond, body } => Stmt::While {
            cond: opt_expr(cond),
            body: body.iter().map(opt_stmt).collect(),
        },
        Stmt::For {
            var,
            iterable,
            body,
        } => Stmt::For {
