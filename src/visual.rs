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
