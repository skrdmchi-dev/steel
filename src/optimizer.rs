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
