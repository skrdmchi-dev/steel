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
    println!("  \x1b[1;32msteel fmt <file.steel> [-w]\x1b[0m    Auto-format a .steel file (--write to save)");
    println!("  \x1b[1;32msteel check <file.steel>\x1b[0m       Run syntax + static type inference check");
    println!("  \x1b[1;32msteel targets\x1b[0m                  List all 8 supported target languages");
    println!("  \x1b[1;32msteel\x1b[0m                          Launch the interactive Steel REPL");
    println!();
    println!("\x1b[1;33mSUPPORTED LANGUAGES (<lang> / --target):\x1b[0m");
    println!("  \x1b[1;36mc\x1b[0m          -> C (C11 with unboxed math/loops + SVal runtime)");
    println!("  \x1b[1;36mcpp | c++\x1b[0m  -> C++ (C++20)");
    println!("  \x1b[1;36mrust | rs\x1b[0m  -> Rust (standalone SVal dynamic runtime)");
    println!("  \x1b[1;36mpython | py\x1b[0m-> Python 3");
    println!("  \x1b[1;36mgo | golang\x1b[0m-> Go (Golang)");
    println!("  \x1b[1;36mjs | node\x1b[0m  -> JavaScript (Node.js)");
    println!("  \x1b[1;36mts\x1b[0m         -> TypeScript");
    println!("  \x1b[1;36mlua\x1b[0m        -> Lua");
    println!("  \x1b[1;36mall\x1b[0m        -> Emit all 8 target languages at once!");
    println!();
    println!("\x1b[1;33mCOMPILER FLAGS (the usual CLI options):\x1b[0m");
    println!("  \x1b[1;35m--visual, -v\x1b[0m         Watch code morph live through all 5 compiler stages");
    println!("  \x1b[1;35m--force, -f\x1b[0m          Force overwrite existing output files");
    println!("  \x1b[1;35m--run, -r\x1b[0m            Immediately execute the compiled output after compiling");
    println!("  \x1b[1;35m--bin, -b\x1b[0m            Compile C/Rust/Go output all the way to a native .exe");
    println!("  \x1b[1;35m--release, -O\x1b[0m        Enable maximum optimizations (-O3 / -C opt-level=3)");
    println!("  \x1b[1;35m--bundle, --single\x1b[0m   Combine multi-file projects into one single output file");
    println!("  \x1b[1;35m--out <path>, -o\x1b[0m     Specify custom output file or directory path");
    println!("  \x1b[1;35m--stdout, --print\x1b[0m    Print generated target code to terminal instead of file");
    println!("  \x1b[1;35m--quiet, -q\x1b[0m          Suppress non-error status messages");
    println!("  \x1b[1;35m--version, -V\x1b[0m        Print Steel compiler version");
    println!();
    println!("\x1b[1;33mEXAMPLES:\x1b[0m");
    println!("  steel compile c app.steel --visual --force");
    println!("  steel compile python app.steel --visual");
    println!("  steel compile rust app.steel --bin --release --force");
    println!("  steel compile all app.steel --force");
}

fn compile_native_binary(src_path: &Path, target: Target, release: bool, quiet: bool) -> Result<String, String> {
    let stem = src_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let parent = src_path.parent().unwrap_or_else(|| Path::new("."));
    let exe_path = parent.join(format!("{}.exe", stem));

    match target {
        Target::Rust => {
            let mut cmd = Command::new("rustc");
            cmd.arg(src_path).arg("-o").arg(&exe_path);
            if release {
                cmd.arg("-C").arg("opt-level=3");
            }
            let status = cmd
                .status()
                .map_err(|e| format!("Failed to invoke rustc: {}", e))?;
            if !status.success() {
                return Err("rustc native binary compilation failed".to_string());
            }
        }
        Target::C => {
            let opt_flag = if release { "-O3" } else { "-O0" };
            let status = Command::new("gcc")
                .arg(src_path)
