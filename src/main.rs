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
                .arg(opt_flag)
                .arg("-o")
                .arg(&exe_path)
                .arg("-lm")
                .status()
                .or_else(|_| {
                    Command::new("clang")
                        .arg(src_path)
                        .arg(opt_flag)
                        .arg("-o")
                        .arg(&exe_path)
                        .status()
                })
                .map_err(|e| format!("Failed to invoke gcc/clang: {}", e))?;
            if !status.success() {
                return Err("C native binary compilation failed".to_string());
            }
        }
        Target::Go => {
            let status = Command::new("go")
                .arg("build")
                .arg("-o")
                .arg(&exe_path)
                .arg(src_path)
                .status()
                .map_err(|e| format!("Failed to invoke go build: {}", e))?;
            if !status.success() {
                return Err("go build failed".to_string());
            }
        }
        other => {
            return Err(format!(
                "--bin is only supported for compiled targets (c, rust, go), not {}",
                other.name()
            ));
        }
    }

    let exe_str = exe_path.to_string_lossy().to_string();
    if !quiet {
        println!(
            "  \x1b[1;32m[BINARY]\x1b[0m Built native executable -> \x1b[1m{}\x1b[0m{}",
            exe_str,
            if release { " (release optimized)" } else { "" }
        );
    }
    Ok(exe_str)
}

fn run_target_code(code: &str, target: Target, stem: &str, release: bool) -> Result<(), String> {
    let tmp_dir = env::temp_dir();
    let src_path = tmp_dir.join(format!("{}_steel_out.{}", stem, target.extension()));
    fs::write(&src_path, code).map_err(|e| format!("Failed to write temp file: {}", e))?;

    let status = match target {
        Target::Js | Target::Ts => Command::new("node").arg(&src_path).status(),
        Target::Python => Command::new("python").arg(&src_path).status(),
        Target::Lua => Command::new("lua").arg(&src_path).status(),
        Target::Go => Command::new("go").arg("run").arg(&src_path).status(),
        Target::Rust => {
            let exe_path = tmp_dir.join(format!("{}_steel_rs.exe", stem));
            let mut cmd = Command::new("rustc");
            cmd.arg(&src_path).arg("-o").arg(&exe_path);
            if release {
                cmd.arg("-C").arg("opt-level=3");
            }
            let compile = cmd
                .status()
                .map_err(|e| format!("Failed to invoke rustc: {}", e))?;
            if !compile.success() {
                return Err("rustc compilation failed".to_string());
            }
            Command::new(&exe_path).status()
        }
        Target::C | Target::Cpp => {
            let exe_path = tmp_dir.join(format!("{}_steel_c.exe", stem));
            let opt_flag = if release { "-O3" } else { "-O0" };
            let compiler = if matches!(target, Target::Cpp) { "g++" } else { "gcc" };
            let compile = Command::new(compiler)
                .arg(&src_path)
                .arg(opt_flag)
                .arg("-o")
                .arg(&exe_path)
                .arg("-lm")
                .status()
                .or_else(|_| {
                    Command::new("clang")
                        .arg(&src_path)
                        .arg(opt_flag)
                        .arg("-o")
                        .arg(&exe_path)
                        .status()
                })
                .map_err(|e| format!("Failed to invoke C/C++ compiler: {}", e))?;
            if !compile.success() {
                return Err("C/C++ compilation failed".to_string());
            }
            Command::new(&exe_path).status()
        }
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("Target process exited with {}", s)),
        Err(e) => Err(format!(
            "Could not launch {} runtime ({}). Tip: run `steel <file.steel>` to use the built-in native interpreter.",
            target.name(),
            e
        )),
    }
}

fn start_repl() {
    println!("\x1b[1;36mSteel v{} Interactive REPL\x1b[0m", VERSION);
    println!("Commands: :target <js|py|rs|c|go|lua|off>, :visual <on|off>, :help, :quit");
    println!("-------------------------------------------------------------------------");

    let mut interp = Interpreter::new();
    let mut preview_target: Option<Target> = None;
    let mut visual_mode = false;
    let stdin = io::stdin();

    loop {
        print!("\x1b[1;32msteel>\x1b[0m ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        if stdin.read_line(&mut line).is_err() || line.is_empty() {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == ":quit" || trimmed == ":q" || trimmed == "exit" {
            break;
        }
        if trimmed == ":help" {
            println!("  :target <js|py|rs|c|go|lua|off>  Live-preview transpiled code in the REPL");
            println!("  :visual <on|off>                 Toggle full 5-stage visual morphing pipeline");
            println!("  :quit                            Exit the REPL");
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix(":visual") {
            let arg = rest.trim();
            visual_mode = arg == "on" || arg == "true" || arg.is_empty();
            println!(
                "Visual compilation morphing: {}",
                if visual_mode { "ENABLED" } else { "DISABLED" }
            );
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix(":target") {
            let arg = rest.trim();
            if arg == "off" || arg == "none" {
                preview_target = None;
                println!("Live transpilation preview disabled.");
            } else if let Some(t) = Target::from_str(arg) {
                preview_target = Some(t);
                println!("Live transpilation preview set to: {}", t.name());
            } else {
                println!("Unknown target '{}'. Choose: js, py, rs, c, go, lua, or off", arg);
            }
            continue;
        }

        match parse_source_full(trimmed) {
            Ok((tokens, prog)) => {
                if visual_mode {
                    run_visual_pipeline("<repl>", trimmed, &tokens, &prog, preview_target);
                } else if let Some(t) = preview_target {
                    println!("--- [{} Transpiled Output] ---", t.name());
                    print!("{}", transpile(&prog, t));
                    println!("------------------------------------");
                }
                match interp.run(&prog) {
                    Ok(Value::Null) => {}
                    Ok(val) => println!("=> {}", val),
                    Err(e) => eprint!("{}", render_diagnostic("<repl>", trimmed, &e)),
                }
            }
            Err(e) => eprint!("{}", render_diagnostic("<repl>", trimmed, &e)),
        }
    }
}

fn load_and_parse(file_path: &str) -> (String, Vec<Token>, Program) {
    let p = Path::new(file_path);
    if p.is_dir() {
        // Support passing a whole directory e.g. `steel compile c ./my-game`!
        let main_candidate = p.join("src").join("main.steel");
        if main_candidate.exists() {
            return load_and_parse(&main_candidate.to_string_lossy());
        }
        let root_main = p.join("main.steel");
        if root_main.exists() {
            return load_and_parse(&root_main.to_string_lossy());
        }
        // Otherwise bundle all `.steel` files in the directory in alphabetical order!
        let mut combined_src = String::new();
        let mut entries: Vec<_> = fs::read_dir(p)
            .unwrap_or_else(|e| {
                eprintln!("\x1b[1;31merror:\x1b[0m Could not read directory '{}': {}", file_path, e);
                std::process::exit(1);
            })
            .flatten()
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("steel"))
            .collect();
        entries.sort_by_key(|e| e.file_name());
        if entries.is_empty() {
            eprintln!("\x1b[1;31merror:\x1b[0m No .steel files found in directory '{}'", file_path);
            std::process::exit(1);
        }
        for entry in entries {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                combined_src.push_str(&content);
                combined_src.push('\n');
            }
        }
        match parse_source_full(&combined_src) {
            Ok((tokens, mut prog)) => {
                let mut visited = std::collections::HashSet::new();
                if let Err(e) = bundle_imports(&mut prog, p, &mut visited) {
                    eprint!("{}", render_diagnostic(file_path, &combined_src, &e));
                    std::process::exit(1);
                }
                return (combined_src, tokens, prog);
            }
            Err(e) => {
                eprint!("{}", render_diagnostic(file_path, &combined_src, &e));
                std::process::exit(1);
            }
        }
    }

    let src = fs::read_to_string(file_path).unwrap_or_else(|e| {
        eprintln!("\x1b[1;31merror:\x1b[0m Could not read '{}': {}", file_path, e);
        std::process::exit(1);
    });
    match parse_source_full(&src) {
        Ok((tokens, mut prog)) => {
            let base_dir = Path::new(file_path)
                .parent()
                .unwrap_or_else(|| Path::new("."));
            let mut visited = std::collections::HashSet::new();
            if let Ok(canon) = fs::canonicalize(file_path) {
                visited.insert(canon);
            }
            if let Err(e) = bundle_imports(&mut prog, base_dir, &mut visited) {
                eprint!("{}", render_diagnostic(file_path, &src, &e));
                std::process::exit(1);
            }
            (src, tokens, prog)
        }
        Err(e) => {
            eprint!("{}", render_diagnostic(file_path, &src, &e));
            std::process::exit(1);
        }
    }
}

/// Recursively resolves and bundles `import "other.steel"` statements into a single Program
/// so multi-file Steel projects compile cleanly to C, C++, Rust, Go, Python, JS, TS, and Lua!
fn bundle_imports(
    prog: &mut Program,
    base_dir: &Path,
    visited: &mut std::collections::HashSet<std::path::PathBuf>,
) -> Result<(), String> {
    let mut bundled = Vec::with_capacity(prog.statements.len());
    for stmt in prog.statements.drain(..) {
        if let Stmt::Import(rel_path) = stmt {
            let candidate = base_dir.join(&rel_path);
            let target_path = if candidate.exists() {
                candidate
            } else {
                std::path::PathBuf::from(&rel_path)
            };
            if let Ok(canon) = fs::canonicalize(&target_path) {
                if !visited.insert(canon) {
                    continue; // Already imported; skip circular/duplicate import
                }
            }
            let imported_src = fs::read_to_string(&target_path)
                .map_err(|e| format!("Failed to import '{}': {}", rel_path, e))?;
            let (_, mut imported_prog) = parse_source_full(&imported_src)?;
            let sub_base = target_path.parent().unwrap_or(base_dir);
            bundle_imports(&mut imported_prog, sub_base, visited)?;
            bundled.append(&mut imported_prog.statements);
        } else {
            bundled.push(stmt);
        }
    }
    prog.statements = bundled;
    Ok(())
}

fn collect_module_recursive(
    file_path: &Path,
    is_entry: bool,
    visited: &mut std::collections::HashSet<std::path::PathBuf>,
    out: &mut Vec<ModuleUnit>,
) -> Result<(), String> {
    let canon = fs::canonicalize(file_path).unwrap_or_else(|_| file_path.to_path_buf());
    if !visited.insert(canon) {
        return Ok(());
    }
    let src = fs::read_to_string(file_path)
        .map_err(|e| format!("Could not read '{}': {}", file_path.display(), e))?;
    let (_, prog) = parse_source_full(&src)?;
    let base_dir = file_path.parent().unwrap_or_else(|| Path::new("."));

    let mut imports = Vec::new();
    for stmt in &prog.statements {
        if let Stmt::Import(rel_path) = stmt {
            let candidate = base_dir.join(rel_path);
            let target_path = if candidate.exists() {
                candidate
            } else {
                std::path::PathBuf::from(rel_path)
            };
            let imp_stem = target_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("module")
                .to_string();
            imports.push(imp_stem);
            collect_module_recursive(&target_path, false, visited, out)?;
        }
    }

    let name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("main")
        .to_string();
    out.push(ModuleUnit {
        name,
        is_entry,
        imports,
        program: prog,
    });
    Ok(())
}

/// Collects a Steel directory or multi-file entry into separate `ModuleUnit`s without combining them.
fn collect_project_modules(path_str: &str) -> Result<Vec<ModuleUnit>, String> {
    let p = Path::new(path_str);
    let mut visited = std::collections::HashSet::new();
    let mut modules = Vec::new();

    if p.is_dir() {
        let entry_path = if p.join("src").join("main.steel").exists() {
            Some(p.join("src").join("main.steel"))
        } else if p.join("main.steel").exists() {
            Some(p.join("main.steel"))
        } else {
            None
        };

        if let Some(ep) = entry_path {
            collect_module_recursive(&ep, true, &mut visited, &mut modules)?;
        }

        // Also include any standalone .steel files in the directory not yet visited
