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
        let mut entries: Vec<_> = fs::read_dir(p)
            .map_err(|e| format!("Could not read directory '{}': {}", path_str, e))?
            .flatten()
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("steel"))
            .collect();
        entries.sort_by_key(|e| e.file_name());
        let has_entry = modules.iter().any(|m| m.is_entry);
        for (idx, entry) in entries.iter().enumerate() {
            let mark_entry = !has_entry && idx == entries.len() - 1;
            collect_module_recursive(&entry.path(), mark_entry, &mut visited, &mut modules)?;
        }
        if modules.is_empty() {
            return Err(format!("No .steel files found in directory '{}'", path_str));
        }
    } else {
        collect_module_recursive(p, true, &mut visited, &mut modules)?;
    }

    Ok(modules)
}

fn compile_native_project_binary(
    out_dir: &Path,
    entry_name: &str,
    target: Target,
    release: bool,
    quiet: bool,
) -> Result<String, String> {
    let exe_path = out_dir.join(format!("{}.exe", entry_name));
    match target {
        Target::Rust => {
            let entry_rs = out_dir.join(format!("{}.rs", entry_name));
            let mut cmd = Command::new("rustc");
            cmd.arg(&entry_rs).arg("-o").arg(&exe_path);
            if release {
                cmd.arg("-C").arg("opt-level=3");
            }
            let status = cmd
                .status()
                .map_err(|e| format!("Failed to invoke rustc: {}", e))?;
            if !status.success() {
                return Err("rustc multi-file native binary compilation failed".to_string());
            }
        }
        Target::C | Target::Cpp => {
            let ext = target.extension();
            let opt_flag = if release { "-O3" } else { "-O0" };
            let src_files: Vec<_> = fs::read_dir(out_dir)
                .map_err(|e| format!("Failed to read '{}': {}", out_dir.display(), e))?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some(ext))
                .collect();
            let compiler = if target == Target::Cpp { "g++" } else { "gcc" };
            let fallback = if target == Target::Cpp { "clang++" } else { "clang" };
            let mut cmd = Command::new(compiler);
            for f in &src_files {
                cmd.arg(f);
            }
            cmd.arg(opt_flag).arg("-o").arg(&exe_path);
            if target == Target::C {
                cmd.arg("-lm");
            } else {
                cmd.arg("-std=c++20");
            }
            let status = cmd
                .status()
                .or_else(|_| {
                    let mut c2 = Command::new(fallback);
                    for f in &src_files {
                        c2.arg(f);
                    }
                    c2.arg(opt_flag).arg("-o").arg(&exe_path);
                    if target == Target::Cpp {
                        c2.arg("-std=c++20");
                    }
                    c2.status()
                })
                .map_err(|e| format!("Failed to invoke {}/{}: {}", compiler, fallback, e))?;
            if !status.success() {
                return Err("C/C++ multi-file native binary compilation failed".to_string());
            }
        }
        Target::Go => {
            let status = Command::new("go")
                .arg("build")
                .arg("-o")
                .arg(&exe_path)
                .arg(".")
                .current_dir(out_dir)
                .status()
                .map_err(|e| format!("Failed to invoke go build: {}", e))?;
            if !status.success() {
                return Err("go build failed".to_string());
            }
        }
        other => {
            return Err(format!(
                "--bin is only supported for compiled targets (c, cpp, rust, go), not {}",
                other.name()
            ));
        }
    }
    let exe_str = exe_path.to_string_lossy().to_string();
    if !quiet {
        println!(
            "  \x1b[1;32m[BIN]\x1b[0m Built native multi-file executable -> \x1b[1;36m{}\x1b[0m",
            exe_str
        );
    }
    Ok(exe_str)
}

fn write_output_file(dest: &str, code: &str, force: bool, quiet: bool, target_name: &str) {
    let path = Path::new(dest);
    if path.exists() && !force {
        eprintln!(
            "\x1b[1;33mwarning:\x1b[0m Output file '{}' already exists. Overwriting (use \x1b[1m--force\x1b[0m / \x1b[1m-f\x1b[0m to suppress this notice).",
            dest
        );
    }
    fs::write(path, code).unwrap_or_else(|e| {
        eprintln!("\x1b[1;31merror:\x1b[0m Failed to write '{}': {}", dest, e);
        std::process::exit(1);
    });
    if !quiet {
        println!(
            "  \x1b[1;32m[OK]\x1b[0m Compiled \x1b[1m{}\x1b[0m -> \x1b[1;36m{}\x1b[0m",
            target_name, dest
        );
    }
}

fn handle_compile_cmd(args: &[String]) {
    let mut file_path: Option<String> = None;
    let mut target_str: Option<String> = None;
    let mut out_path: Option<String> = None;
    let mut visual = false;
    let mut force = false;
    let mut run_after = false;
    let mut build_bin = false;
    let mut release = false;
    let mut to_stdout = false;
    let mut quiet = false;
    let mut bundle = false;

    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--visual" | "-v" => visual = true,
            "--force" | "-f" => force = true,
            "--run" | "-r" => run_after = true,
            "--bin" | "-b" => build_bin = true,
            "--release" | "-O" => release = true,
            "--stdout" | "--print" | "-p" => to_stdout = true,
            "--quiet" | "-q" => quiet = true,
            "--bundle" | "--single" => bundle = true,
            "-t" | "--target" | "-l" | "--lang" => {
                if let Some(val) = args.get(i + 1) {
                    target_str = Some(val.clone());
                    i += 1;
                } else {
                    eprintln!("\x1b[1;31merror:\x1b[0m Missing language after {}", arg);
                    std::process::exit(1);
                }
            }
            "-o" | "--out" | "--output" => {
                if let Some(val) = args.get(i + 1) {
                    out_path = Some(val.clone());
                    i += 1;
                } else {
                    eprintln!("\x1b[1;31merror:\x1b[0m Missing file path after {}", arg);
                    std::process::exit(1);
                }
            }
            "--" => {
                // Ignore bare `--` separator so `steel compile c file.steel -- --visual --force` works seamlessly!
            }
            other if other.starts_with("--target=") => {
                target_str = Some(other.trim_start_matches("--target=").to_string());
            }
            other if other.starts_with("--out=") => {
                out_path = Some(other.trim_start_matches("--out=").to_string());
            }
            other if other.starts_with('-') => {
                // Support combined short flags like `-vf` or `-vfr`
                if other.len() > 1 && !other.starts_with("--") {
                    for ch in other.chars().skip(1) {
                        match ch {
                            'v' => visual = true,
                            'f' => force = true,
                            'r' => run_after = true,
                            'b' => build_bin = true,
                            'O' => release = true,
                            'p' => to_stdout = true,
                            'q' => quiet = true,
                            _ => {
                                eprintln!("\x1b[1;31merror:\x1b[0m Unknown short flag '-{}'", ch);
                                std::process::exit(1);
                            }
                        }
                    }
                } else {
                    eprintln!(
                        "\x1b[1;31merror:\x1b[0m Unknown flag '{}'. Run `steel --help` for options.",
                        other
                    );
                    std::process::exit(1);
                }
            }
            positional => {
                if Target::from_str(positional).is_some()
                    || positional.eq_ignore_ascii_case("all")
                {
                    if target_str.is_none() {
                        target_str = Some(positional.to_string());
                    } else if file_path.is_none() {
                        file_path = Some(positional.to_string());
                    }
                } else if file_path.is_none() {
                    file_path = Some(positional.to_string());
                } else if target_str.is_none() {
                    target_str = Some(positional.to_string());
                }
            }
        }
        i += 1;
    }

    let file_path = file_path.unwrap_or_else(|| {
        eprintln!("Usage: steel compile <lang> <file.steel> [--visual] [--force] [--run] [--bin] [--release] [-o <out>]");
        eprintln!("   or: steel compile <file.steel> <lang> [flags]");
        std::process::exit(1);
    });
    let target_str = target_str.unwrap_or_else(|| "c".to_string());
    let trimmed_path = file_path.trim_end_matches(&['/', '\\'][..]);
    let base = trimmed_path.strip_suffix(".steel").unwrap_or(trimmed_path);

    // Multi-file project output is the DEFAULT whenever compiling a directory or a file with imports.
    // Pass `--bundle` (or `--single`) to combine all files into one single output file.
    if !bundle && !to_stdout {
        let is_dir = Path::new(trimmed_path).is_dir();
        if let Ok(modules) = collect_project_modules(trimmed_path) {
            if is_dir || modules.len() > 1 {
                let targets_to_emit: Vec<Target> = if target_str.eq_ignore_ascii_case("all") {
                    vec![
                        Target::Js,
                        Target::Ts,
                        Target::Python,
                        Target::Rust,
                        Target::C,
                        Target::Cpp,
                        Target::Go,
                        Target::Lua,
                    ]
                } else {
                    let t = Target::from_str(&target_str).unwrap_or_else(|| {
                        eprintln!(
                            "\x1b[1;31merror:\x1b[0m Unknown target language '{}'. Supported: c, cpp, rust (rs), python (py), go, js, ts, lua, all",
                            target_str
                        );
                        std::process::exit(1);
                    });
                    vec![t]
                };

                let entry_name = modules
                    .iter()
                    .find(|m| m.is_entry)
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| "main".to_string());

                for target in targets_to_emit {
                    let out_dir_str = if target_str.eq_ignore_ascii_case("all") {
                        format!("{}_{}", base, target.extension())
                    } else {
                        out_path
                            .clone()
                            .unwrap_or_else(|| format!("{}_{}", base, target.extension()))
                    };
                    let out_dir = Path::new(&out_dir_str);
                    if let Err(e) = fs::create_dir_all(out_dir) {
                        eprintln!(
                            "\x1b[1;31merror:\x1b[0m Could not create output directory '{}': {}",
                            out_dir_str, e
                        );
                        std::process::exit(1);
                    }

                    let files = transpile_project(&modules, target);
                    for (rel_file, code) in &files {
                        let dest_path = out_dir.join(rel_file);
                        write_output_file(
                            &dest_path.to_string_lossy(),
                            code,
                            force,
                            quiet,
                            target.name(),
                        );
                    }

                    if !quiet {
                        println!(
                            "  \x1b[1;36m[PROJECT]\x1b[0m Emitted {} separate {} files into \x1b[1;36m{}/\x1b[0m (use --bundle to combine into one file)",
                            files.len(),
                            target.name(),
                            out_dir_str
                        );
                    }

                    if build_bin && !target_str.eq_ignore_ascii_case("all") {
                        if let Err(e) = compile_native_project_binary(
                            out_dir,
                            &entry_name,
                            target,
                            release,
                            quiet,
                        ) {
                            eprintln!("\x1b[1;31merror:\x1b[0m {}", e);
                            std::process::exit(1);
                        }
                    }

                    if run_after && !target_str.eq_ignore_ascii_case("all") {
                        let (_src, _tokens, prog) = load_and_parse(trimmed_path);
                        let bundled_code = transpile(&prog, target);
                        if !quiet {
                            println!("  \x1b[1;36m[RUN]\x1b[0m Executing {} output...", target.name());
                        }
                        if let Err(e) =
                            run_target_code(&bundled_code, target, "steel_compile_run", release)
                        {
                            eprintln!("\x1b[1;31merror:\x1b[0m {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                return;
            }
        }
    }

    let (src, tokens, prog) = load_and_parse(&file_path);

    if target_str.eq_ignore_ascii_case("all") {
        if visual {
            run_visual_pipeline(&file_path, &src, &tokens, &prog, Some(Target::C));
        }
        for t in [
            Target::Js,
            Target::Ts,
            Target::Python,
            Target::Rust,
            Target::C,
            Target::Cpp,
            Target::Go,
            Target::Lua,
        ] {
            let code = transpile(&prog, t);
            let dest = format!("{}.{}", base, t.extension());
            write_output_file(&dest, &code, force, quiet, t.name());
        }
        return;
    }

    let target = Target::from_str(&target_str).unwrap_or_else(|| {
        eprintln!(
            "\x1b[1;31merror:\x1b[0m Unknown target language '{}'. Supported: c, rust (rs), python (py), go, js, lua, all",
            target_str
        );
        std::process::exit(1);
    });

    if visual {
        run_visual_pipeline(&file_path, &src, &tokens, &prog, Some(target));
    }

    let code = transpile(&prog, target);

    if to_stdout {
        print!("{}", code);
    } else {
        let dest = out_path.unwrap_or_else(|| format!("{}.{}", base, target.extension()));
        write_output_file(&dest, &code, force, quiet, target.name());

        if build_bin {
            if let Err(e) = compile_native_binary(Path::new(&dest), target, release, quiet) {
                eprintln!("\x1b[1;31merror:\x1b[0m {}", e);
                std::process::exit(1);
            }
        }
    }

    if run_after {
        if !quiet {
            println!("  \x1b[1;36m[RUN]\x1b[0m Executing {} output...", target.name());
        }
        if let Err(e) = run_target_code(&code, target, "steel_compile_run", release) {
            eprintln!("\x1b[1;31merror:\x1b[0m {}", e);
            std::process::exit(1);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        start_repl();
        return;
    }

    let visual = args.iter().any(|a| a == "--visual" || a == "-v");
    let cmd = args[0].as_str();

    match cmd {
        "-h" | "--help" | "help" => {
            print_help();
        }
        "-V" | "--version" | "version" => {
            println!("steel {} (multi-target compiler & VM)", VERSION);
        }
        "targets" => {
            println!("Available Steel Target Languages (8 Backends):");
            for t in [
                Target::C,
                Target::Cpp,
                Target::Rust,
                Target::Python,
                Target::Go,
                Target::Js,
                Target::Ts,
                Target::Lua,
            ] {
                println!("  - {:<22} (.{})", t.name(), t.extension());
            }
        }
        "init" | "new" => {
            let proj_name = args.get(1).map(|s| s.as_str()).unwrap_or("steel-app");
            let proj_dir = Path::new(proj_name);
            let src_dir = proj_dir.join("src");
            let tests_dir = proj_dir.join("tests");
            let _ = fs::create_dir_all(&src_dir);
            let _ = fs::create_dir_all(&tests_dir);

            let toml = format!(
                "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nentry = \"src/main.steel\"\ntests = \"tests/main_test.steel\"\ndefault_target = \"c\"\n",
                proj_name
            );
            let main_steel = format!(
                "// {} — Entry Point (src/main.steel)\n\nfn greet(name: str) -> str {{\n    return \"Welcome to {}, ${{name}}!\"\n}}\n\nprint(greet(\"Developer\"))\n",
                proj_name, proj_name
            );
            let test_steel = "// Unit Tests (run with `steel test`)\n\ntest \"sample project test\" {\n    let nums = [1, 2, 3, 4]\n    assert(nums.sum() == 10, \"Sum should be 10\")\n}\n";

            let _ = fs::write(proj_dir.join("steel.toml"), toml);
            let _ = fs::write(src_dir.join("main.steel"), main_steel);
            let _ = fs::write(tests_dir.join("main_test.steel"), test_steel);
            println!(
                "  \x1b[1;32m[CREATED]\x1b[0m Scaffolded new Steel project in \x1b[1;36m{}/\x1b[0m",
                proj_name
            );
            println!("    - {}/steel.toml", proj_name);
            println!("    - {}/src/main.steel", proj_name);
            println!("    - {}/tests/main_test.steel", proj_name);
            println!("\n  Get started:\n    cd {}\n    steel run\n    steel test", proj_name);
        }
        "watch" => {
            let file_path = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| "src/main.steel".to_string());
            println!(
                "\x1b[1;36m[WATCH]\x1b[0m Watching '\x1b[1m{}\x1b[0m' for changes (Ctrl+C to stop)...",
                file_path
            );
            let mut last_content = String::new();
            loop {
                if let Ok(content) = fs::read_to_string(&file_path) {
                    if content != last_content {
                        last_content = content.clone();
                        println!("\x1b[1;33m--- [Change detected: Running {}] ---\x1b[0m", file_path);
                        let t0 = Instant::now();
                        match parse_source_full(&content) {
                            Ok((tokens, prog)) => {
                                let opt_prog = optimize_program(&prog);
                                if visual {
                                    run_visual_pipeline(&file_path, &content, &tokens, &opt_prog, None);
                                }
                                let mut interp = Interpreter::new();
                                match interp.run(&opt_prog) {
                                    Ok(_) => println!(
                                        "\x1b[1;32m[OK]\x1b[0m Finished in {:.2?}",
                                        t0.elapsed()
                                    ),
                                    Err(e) => eprint!("{}", render_diagnostic(&file_path, &content, &e)),
                                }
                            }
                            Err(e) => eprint!("{}", render_diagnostic(&file_path, &content, &e)),
                        }
                    }
                }
                thread::sleep(Duration::from_millis(350));
            }
        }
        "bench" | "benchmark" => {
            let file_path = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| "examples/showcase.steel".to_string());
            let (src, tokens, prog) = load_and_parse(&file_path);

            let t_opt0 = Instant::now();
            let opt_prog = optimize_program(&prog);
            let d_opt = t_opt0.elapsed();

            let t_vm_comp0 = Instant::now();
            let chunk = compile_program(&opt_prog);
            let d_vm_comp = t_vm_comp0.elapsed();

            let t_vm_exec0 = Instant::now();
            let _ = chunk.execute(true);
            let d_vm_exec = t_vm_exec0.elapsed();

            let t_c_gen0 = Instant::now();
            let c_code = transpile(&opt_prog, Target::C);
            let d_c_gen = t_c_gen0.elapsed();

            println!("\x1b[1;36m=========================================================================\x1b[0m");
            println!("\x1b[1;37m  STEEL MULTI-ENGINE BENCHMARK SUITE — {}\x1b[0m", file_path);
            println!("\x1b[1;36m=========================================================================\x1b[0m");
            println!("  Source Metrics     : {} bytes | {} tokens | {} AST statements", src.len(), tokens.len(), prog.statements.len());
            println!("  AST Constant Fold  : \x1b[1;32m{:.2?}\x1b[0m", d_opt);
            println!("  Bytecode Compile   : \x1b[1;32m{:.2?}\x1b[0m ({} VM instructions)", d_vm_comp, chunk.ops.len());
            println!("  Stack VM Execution : \x1b[1;32m{:.2?}\x1b[0m (headless fast-path)", d_vm_exec);
            println!("  C11 Codegen Pass   : \x1b[1;32m{:.2?}\x1b[0m ({} bytes emitted)", d_c_gen, c_code.len());
            println!("\x1b[1;36m=========================================================================\x1b[0m");
        }
        "doc" | "docs" => {
            if args.len() < 2 {
                eprintln!("Usage: steel doc <file.steel> [-o <DOCS.md>]");
                std::process::exit(1);
            }
            let file_path = &args[1];
            let (_src, _tokens, prog) = load_and_parse(file_path);
            let report = typecheck::analyze_program(&prog);
            let mut md = format!("# API Documentation — `{}`\n\n", file_path);
            md.push_str("## Inferred Symbol Table\n\n| Symbol | Inferred Type |\n| :--- | :--- |\n");
            for (sym, ty) in &report.symbols {
                md.push_str(&format!("| `{}` | `{}` |\n", sym, ty));
            }
            md.push_str("\n## Declarations\n\n");
            for st in &prog.statements {
                match st {
                    Stmt::FnDecl { name, params, .. } => {
                        let p_list: Vec<String> = params
                            .iter()
                            .map(|(n, d)| match d {
                                Some(e) => format!("{} = {:?}", n, e),
                                None => n.clone(),
                            })
                            .collect();
                        md.push_str(&format!("### `fn {}({})`\n\n", name, p_list.join(", ")));
                    }
                    Stmt::ClassDecl {
                        name,
                        parent,
                        methods,
                    } => {
                        let ext = parent
                            .as_ref()
                            .map(|p| format!(" extends {}", p))
                            .unwrap_or_default();
                        md.push_str(&format!("### `class {}{}`\n", name, ext));
                        for m in methods {
                            let p_list: Vec<String> =
                                m.params.iter().map(|(n, _)| n.clone()).collect();
                            md.push_str(&format!("- Method `{}`(`{}`)\n", m.name, p_list.join(", ")));
                        }
                        md.push('\n');
                    }
                    _ => {}
                }
            }
            if let Some(pos) = args.iter().position(|a| a == "-o" || a == "--out") {
                if let Some(dest) = args.get(pos + 1) {
                    let _ = fs::write(dest, &md);
                    println!("  \x1b[1;32m[DOC]\x1b[0m Generated Markdown docs -> {}", dest);
                    return;
                }
            }
            print!("{}", md);
        }
        "compile" | "transpile" | "build" | "c" => {
            handle_compile_cmd(&args);
        }
        "check" => {
            if args.len() < 2 {
                eprintln!("Usage: steel check <file.steel>");
                std::process::exit(1);
            }
            let file_path = &args[1];
            let (src, tokens, prog) = load_and_parse(file_path);
            let report = typecheck::analyze_program(&prog);
            if !report.errors.is_empty() {
                for err in &report.errors {
                    eprint!("{}", render_diagnostic(file_path, &src, err));
                }
                std::process::exit(1);
            }
            println!(
                "\x1b[1;32m[OK]\x1b[0m '{}' passed syntax & type analysis ({} tokens, {} statements, {} inferred symbols)",
                file_path,
                tokens.len(),
                prog.statements.len(),
                report.symbols.len()
            );
            for (sym, ty) in &report.symbols {
                println!("  - {:<18} : {}", sym, ty);
            }
        }
        "disasm" | "vm" => {
            if args.len() < 2 {
                eprintln!("Usage: steel vm <file.steel> [--run]");
                std::process::exit(1);
            }
            let file_path = &args[1];
            let (_src, _tokens, prog) = load_and_parse(file_path);
            let opt_prog = optimize_program(&prog);
            let chunk = compile_program(&opt_prog);
            if args.iter().any(|a| a == "--run" || a == "-r") {
                match chunk.execute(false) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("VM Runtime Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                print!("{}", chunk.disassemble());
            }
        }
        "fmt" | "format" => {
            if args.len() < 2 {
                eprintln!("Usage: steel fmt <file.steel> [--write]");
                std::process::exit(1);
            }
            let file_path = &args[1];
            let write_in_place = args.iter().any(|a| a == "--write" || a == "-w");
            let (_src, _tokens, prog) = load_and_parse(file_path);
            let formatted = format_program(&prog);
            if write_in_place {
                fs::write(file_path, formatted).unwrap_or_else(|e| {
                    eprintln!("Failed to write '{}': {}", file_path, e);
                    std::process::exit(1);
                });
                println!("Formatted '{}'", file_path);
            } else {
                print!("{}", formatted);
            }
        }
        "test" => {
            let file_path = args.get(1).cloned().unwrap_or_else(|| {
                if Path::new("tests/main_test.steel").exists() {
                    "tests/main_test.steel".to_string()
                } else {
                    "examples/tests.steel".to_string()
                }
            });
            let (src, _tokens, prog) = load_and_parse(&file_path);
            let prog = optimize_program(&prog);
            let mut setup_stmts = Vec::new();
            let mut tests = Vec::new();

            for st in &prog.statements {
                if let Stmt::Test { name, body } = st {
                    tests.push((name.clone(), body.clone()));
                } else {
                    setup_stmts.push(st.clone());
                }
            }

            println!("Running {} test(s) from '{}'...", tests.len(), file_path);
            let mut passed = 0;
            let mut failed = 0;
            let start_all = Instant::now();

            for (t_name, t_body) in tests {
                let mut interp = Interpreter::new();
                let _ = interp.run(&Program {
                    statements: setup_stmts.clone(),
                });
                let t0 = Instant::now();
                match interp.run(&Program { statements: t_body }) {
                    Ok(_) => {
                        passed += 1;
                        println!(
                            "  \x1b[1;32m[PASS]\x1b[0m {} ({:.2?})",
                            t_name,
                            t0.elapsed()
                        );
                    }
                    Err(e) => {
                        failed += 1;
                        println!("  \x1b[1;31m[FAIL]\x1b[0m {}", t_name);
                        eprint!("{}", render_diagnostic(&file_path, &src, &e));
                    }
                }
            }
            println!(
                "\nTest Result: \x1b[1;32m{} passed\x1b[0m, \x1b[1;31m{} failed\x1b[0m (finished in {:.2?})",
                passed,
                failed,
                start_all.elapsed()
            );
            if failed > 0 {
                std::process::exit(1);
            }
        }
        "run" => {
            let file_path = args.get(1).cloned().unwrap_or_else(|| {
                if Path::new("src/main.steel").exists() {
                    "src/main.steel".to_string()
                } else {
                    eprintln!("Usage: steel run <file.steel> [--target <native|js|ts|py|rs|c|cpp|go|lua>] [--visual] [--release]");
                    std::process::exit(1);
                }
            });
