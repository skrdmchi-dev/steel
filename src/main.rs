mod ast;
mod codegen;
mod codegen_cpp;
mod codegen_ts;
mod diag;
mod formatter;
mod interp;
mod lexer;
mod optimizer;
mod parser;
mod typecheck;
mod visual;
mod vm;

use ast::{Program, Stmt};
use codegen::{transpile, transpile_project, ModuleUnit, Target};
use diag::render_diagnostic;
use formatter::format_program;
use interp::{Interpreter, Value};
use lexer::{Lexer, Token};
use optimizer::optimize_program;
use parser::Parser;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};
use visual::run_visual_pipeline;
use vm::compile_program;

const VERSION: &str = "1.0.0";

fn parse_source_full(src: &str) -> Result<(Vec<Token>, Program), String> {
    let mut lexer = Lexer::new(src);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens.clone());
    let prog = parser.parse_program()?;
    Ok((tokens, prog))
}

fn print_help() {
    println!("\x1b[1;36m=============================================================================\x1b[0m");
    println!("\x1b[1;37m  STEEL (.steel) v{} - Multi-Target Compiler, VM & Visual Toolchain\x1b[0m", VERSION);
    println!("\x1b[1;36m=============================================================================\x1b[0m");
    println!();
    println!("\x1b[1;33mPRIMARY COMMANDS:\x1b[0m");
    println!("  \x1b[1;32msteel compile <lang> <file.steel> [flags]\x1b[0m");
    println!("      Compile/transpile a .steel file to a target language (or native binary)");
    println!("      (Also supports: \x1b[36msteel compile <file.steel> <lang> [flags]\x1b[0m)");
    println!();
    println!("  \x1b[1;32msteel run <file.steel> [flags]\x1b[0m");
    println!("      Execute a .steel file directly (or via a target compiler with -t <lang>)");
    println!();
    println!("  \x1b[1;32msteel init [name]\x1b[0m              Scaffold a new Steel project (steel.toml, src/, tests/)");
    println!("  \x1b[1;32msteel watch <file.steel>\x1b[0m       Live hot-reload watcher (re-runs on Ctrl+S save)");
    println!("  \x1b[1;32msteel bench <file.steel>\x1b[0m       Benchmark AST Optimizer, Interpreter & Stack VM");
    println!("  \x1b[1;32msteel doc <file.steel>\x1b[0m         Generate Markdown API docs from functions & classes");
    println!("  \x1b[1;32msteel vm <file.steel> [--run]\x1b[0m  Disassemble or execute Stack VM Bytecode directly");
    println!("  \x1b[1;32msteel test [file.steel]\x1b[0m        Run all `test \"...\" {{ ... }}` unit test blocks");
