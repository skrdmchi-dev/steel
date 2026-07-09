use crate::ast::{Expr, Program, Stmt};
use crate::codegen::{transpile, Target};
use crate::lexer::{Token, TokenKind};
use crate::typecheck::analyze_program;
use crate::vm::compile_program;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn step_pause(ms: u64) {
    let _ = io::stdout().flush();
    thread::sleep(Duration::from_millis(ms));
}

fn banner(stage_num: usize, title: &str, color: &str) {
    println!();
    println!(
        "{}=========================================================================\x1b[0m",
        color
    );
    println!(
        "{}  [STAGE {}/5] {}\x1b[0m",
        color, stage_num, title
    );
    println!(
        "{}=========================================================================\x1b[0m",
        color
    );
    step_pause(90);
}

pub fn run_visual_pipeline(
    file_path: &str,
    src: &str,
    tokens: &[Token],
    program: &Program,
    target: Option<Target>,
) {
    println!("\x1b[1;35m");
    println!("   _____ _______ ______ ______ _        __      _______  _____ _    _          _      ");
    println!("  / ____|__   __|  ____|  ____| |       \\ \\    / /_   _|/ ____| |  | |   /\\   | |     ");
    println!(" | (___    | |  | |__  | |__  | |        \\ \\  / /  | | | (___ | |  | |  /  \\  | |     ");
    println!("  \\___ \\   | |  |  __| |  __| | |         \\ \\/ /   | |  \\___ \\| |  | | / /\\ \\ | |     ");
    println!("  ____) |  | |  | |____| |____| |____      \\  /   _| |_ ____) | |__| |/ ____ \\| |____ ");
    println!(" |_____/   |_|  |______|______|______|      \\/   |_____|_____/ \\____//_/    \\_\\______|");
    println!("\x1b[0m");
    println!(
        "  \x1b[1mSource File:\x1b[0m {} ({} bytes, {} lines)",
        file_path,
        src.len(),
        src.lines().count()
    );
    step_pause(120);

    // -------------------------------------------------------------------------
    // STAGE 1: LEXICAL ANALYSIS (TOKEN STREAM)
    // -------------------------------------------------------------------------
    banner(
        1,
        "LEXICAL ANALYSIS (Source Code -> Token Stream)",
        "\x1b[1;36m",
    );
    let max_preview = 28.min(tokens.len());
    print!("  ");
    for (i, tok) in tokens.iter().take(max_preview).enumerate() {
        if matches!(tok.kind, TokenKind::Eof) {
            print!("\x1b[1;90m[EOF]\x1b[0m");
            break;
        }
        let badge = match &tok.kind {
            TokenKind::Let | TokenKind::Const | TokenKind::Fn | TokenKind::Return
            | TokenKind::If | TokenKind::Else | TokenKind::While | TokenKind::For
            | TokenKind::In | TokenKind::Match | TokenKind::Class | TokenKind::New
            | TokenKind::Try | TokenKind::Catch | TokenKind::Throw | TokenKind::Test => {
                format!("\x1b[1;35m[{:?}]\x1b[0m", tok.kind)
            }
            TokenKind::Ident(s) => format!("\x1b[1;36m[Id:{}]\x1b[0m", s),
            TokenKind::Number(n) => format!("\x1b[1;33m[Num:{}]\x1b[0m", n),
            TokenKind::String(s) => {
                let short = if s.len() > 12 { &s[..12] } else { s };
                format!("\x1b[1;32m[Str:{:?}]\x1b[0m", short)
            }
            other => format!("\x1b[90m[{:?}]\x1b[0m", other),
        };
        print!("{} ", badge);
        if (i + 1) % 6 == 0 {
            println!();
            print!("  ");
        }
        step_pause(18);
    }
    if tokens.len() > max_preview {
        println!(
            "\n  \x1b[90m... +{} more tokens ({} total tokens)\x1b[0m",
            tokens.len() - max_preview,
            tokens.len()
        );
    } else {
        println!();
    }

    // -------------------------------------------------------------------------
    // STAGE 2: ABSTRACT SYNTAX TREE (AST)
    // -------------------------------------------------------------------------
    banner(
        2,
        "PARSER & ABSTRACT SYNTAX TREE (Tokens -> AST)",
        "\x1b[1;33m",
    );
    println!("  \x1b[1;33mProgram\x1b[0m ({} top-level statements)", program.statements.len());
    let max_stmts = 14.min(program.statements.len());
    for (i, stmt) in program.statements.iter().take(max_stmts).enumerate() {
        let is_last = i + 1 == max_stmts && program.statements.len() == max_stmts;
        let branch = if is_last { "└──" } else { "├──" };
        println!("  {} {}", branch, summarize_stmt(stmt));
        step_pause(30);
    }
    if program.statements.len() > max_stmts {
        println!(
            "  └── \x1b[90m... (+{} more AST nodes)\x1b[0m",
            program.statements.len() - max_stmts
        );
    }

    // -------------------------------------------------------------------------
    // STAGE 3: STATIC TYPE INFERENCE & SYMBOL TABLE
    // -------------------------------------------------------------------------
    banner(
        3,
        "STATIC TYPE INFERENCE & UNBOXING PASS (AST -> Typed IR)",
        "\x1b[1;32m",
    );
    let report = analyze_program(program);
    if report.symbols.is_empty() {
        println!("  (No top-level symbols declared)");
    } else {
        println!(
            "  \x1b[1m{:<20} | {:<28} | {}\x1b[0m",
            "SYMBOL", "INFERRED TYPE", "CODEGEN STRATEGY"
        );
        println!("  ---------------------+------------------------------+-------------------------");
        for (name, ty) in report.symbols.iter().take(16) {
            let strategy = match ty {
                crate::typecheck::InferredType::Num => "\x1b[32mUnboxed Native f64/double\x1b[0m",
                crate::typecheck::InferredType::Bool => "\x1b[32mUnboxed Native bool\x1b[0m",
                crate::typecheck::InferredType::Str => "\x1b[36mString Slice / const char*\x1b[0m",
                crate::typecheck::InferredType::Func { .. } => "\x1b[35mDirect Function Symbol\x1b[0m",
                crate::typecheck::InferredType::Class(_) => "\x1b[33mClass Constructor + Map\x1b[0m",
                _ => "\x1b[90mDynamic Boxed SVal\x1b[0m",
            };
            println!("  {:<20} | {:<28} | {}", name, ty.to_string(), strategy);
            step_pause(25);
        }
    }

    // -------------------------------------------------------------------------
    // STAGE 4: STACK VM BYTECODE DISASSEMBLY
    // -------------------------------------------------------------------------
    banner(
        4,
        "STEEL VIRTUAL MACHINE BYTECODE (AST -> Stack VM Instructions)",
        "\x1b[1;34m",
    );
    let chunk = compile_program(program);
    let disasm = chunk.disassemble();
    for (idx, line) in disasm.lines().take(22).enumerate() {
        if line.starts_with("===") {
            println!("  \x1b[1;34m{}\x1b[0m", line);
        } else {
            println!("  {}", line);
        }
        if idx % 2 == 0 {
            step_pause(15);
        }
    }
    let total_lines = disasm.lines().count();
    if total_lines > 22 {
        println!("  \x1b[90m... (+{} more bytecode instructions)\x1b[0m", total_lines - 22);
    }

    // -------------------------------------------------------------------------
    // STAGE 5: TARGET CODE GENERATION (LIVE MORPHING!)
    // -------------------------------------------------------------------------
    let chosen_target = target.unwrap_or(Target::C);
    banner(
        5,
        &format!(
            "TARGET CODE GENERATION (Steel AST -> {})",
            chosen_target.name()
        ),
        "\x1b[1;35m",
    );
    let generated = transpile(program, chosen_target);
    let gen_lines: Vec<&str> = generated.lines().collect();
    // Skip runtime header to show the user's morphed functions & main body
    let user_code_start = gen_lines
        .iter()
        .position(|l| {
            l.starts_with("SVal ")
                || l.starts_with("fn ")
                || l.starts_with("func ")
                || l.starts_with("def ")
                || l.starts_with("function ")
                || l.starts_with("local function ")
                || l.starts_with("int main")
        })
        .unwrap_or(0);

    println!(
        "  \x1b[90m[Embedded Steel Runtime Header: {} lines]\x1b[0m",
        user_code_start
    );
    for (i, line) in gen_lines
        .iter()
        .skip(user_code_start)
        .take(32)
        .enumerate()
    {
        println!(
            "  \x1b[90m{:03} |\x1b[0m \x1b[1;37m{}\x1b[0m",
            user_code_start + i + 1,
            line
        );
        step_pause(22);
    }
    if gen_lines.len() > user_code_start + 32 {
        println!(
            "  \x1b[90m... (+{} more lines of generated {})\x1b[0m",
            gen_lines.len() - (user_code_start + 32),
            chosen_target.name()
        );
    }
    println!(
        "\x1b[1;32m=========================================================================\x1b[0m"
    );
    println!(
        "\x1b[1;32m  [VISUAL PIPELINE COMPLETE] Steel -> {} morphing finished!\x1b[0m",
        chosen_target.name()
    );
    println!(
        "\x1b[1;32m=========================================================================\x1b[0m\n"
    );
}

fn summarize_stmt(stmt: &Stmt) -> String {
    match stmt {
        Stmt::Let {
            name,
            is_const,
            value,
        } => {
            let kw = if *is_const { "ConstDecl" } else { "LetDecl" };
            format!(
                "\x1b[1;36m{}\x1b[0m({} = {})",
                kw,
                name,
                summarize_expr(value)
            )
        }
        Stmt::Assign { target, value } => {
            format!("\x1b[1;36mAssign\x1b[0m({:?} = {})", target, summarize_expr(value))
        }
        Stmt::FnDecl { name, params, body } => {
            let p: Vec<String> = params.iter().map(|(n, _)| n.clone()).collect();
            format!(
