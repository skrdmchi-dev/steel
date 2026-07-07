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
