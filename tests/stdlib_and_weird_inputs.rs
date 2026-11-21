use std::fs;
use std::path::Path;
use std::process::Command;

fn steel_bin() -> &'static str {
    env!("CARGO_BIN_EXE_steel")
}

fn has_cmd(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().is_ok()
}

fn run_all_9_runtimes(test_name: &str, src: &str) {
    let tmp = std::env::temp_dir().join(format!("steel_test_{}", test_name));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let steel_file = tmp.join("app.steel");
    fs::write(&steel_file, src).unwrap();

    // 1. Steel interpreter (reference)
    let run_steel = Command::new(steel_bin())
        .args(["run", steel_file.to_str().unwrap()])
        .output()
        .expect("steel run failed");
    assert!(
        run_steel.status.success(),
        "[{}] steel run failed:\nSTDOUT:\n{}\nSTDERR:\n{}",
        test_name,
        String::from_utf8_lossy(&run_steel.stdout),
        String::from_utf8_lossy(&run_steel.stderr)
    );
    let expected = String::from_utf8_lossy(&run_steel.stdout)
        .replace("\r\n", "\n")
        .trim()
        .to_string();

    // 2. TypeScript (strict tsc --noEmit + node)
    let ts_dir = tmp.join("ts_out");
    fs::create_dir_all(&ts_dir).unwrap();
    let ts_file = ts_dir.join("app.ts");
    let ts_comp = Command::new(steel_bin())
        .args([
            "compile",
            steel_file.to_str().unwrap(),
            "--target",
            "ts",
            "-o",
            ts_file.to_str().unwrap(),
            "--force",
        ])
        .output()
        .expect("ts compile failed");
    assert!(
        ts_comp.status.success(),
        "[{}] ts compile failed: {}",
        test_name,
        String::from_utf8_lossy(&ts_comp.stderr)
    );
    let ts_code = fs::read_to_string(&ts_file).unwrap();
    assert!(!ts_code.contains("@ts-nocheck"), "[{}] TS emitted @ts-nocheck", test_name);
    fs::write(
        ts_dir.join("tsconfig.json"),
        r#"{"compilerOptions":{"target":"ES2022","module":"NodeNext","moduleResolution":"NodeNext","strict":true,"noEmit":true},"include":["./**/*.ts"]}"#,
    )
    .unwrap();
    fs::write(
        ts_dir.join("env.d.ts"),
        "declare module \"node:fs\" { export function readFileSync(p: string, e: string): string; export function writeFileSync(p: string, d: string): void; export function appendFileSync(p: string, d: string): void; export function unlinkSync(p: string): void; export function existsSync(p: string): boolean; }\ndeclare const process: { env: Record<string, string | undefined> };\n",
    )
    .unwrap();
    if has_cmd("tsc.cmd") {
        let tsc = Command::new("tsc.cmd")
            .args(["--noEmit", "-p", ts_dir.join("tsconfig.json").to_str().unwrap()])
            .output()
            .expect("tsc failed");
        assert!(
            tsc.status.success(),
            "[{}] tsc --noEmit failed:\n{}\n{}\n--- TS ---\n{}",
            test_name,
            String::from_utf8_lossy(&tsc.stdout),
            String::from_utf8_lossy(&tsc.stderr),
            ts_code
        );
    }
    if has_cmd("node") {
        let run_ts = Command::new("node").arg(&ts_file).output().unwrap();
        assert!(
            run_ts.status.success(),
            "[{}] node ts failed:\n{}",
            test_name,
            String::from_utf8_lossy(&run_ts.stderr)
        );
        let got = String::from_utf8_lossy(&run_ts.stdout).replace("\r\n", "\n").trim().to_string();
        assert_eq!(got, expected, "[{}] TS output mismatch", test_name);
    }

    // 3. C++20 (-std=c++20 -O2 -Wall -Wextra -Wpedantic)
    let cpp_file = tmp.join("app.cpp");
    let cpp_comp = Command::new(steel_bin())
        .args([
            "compile",
            steel_file.to_str().unwrap(),
            "--target",
            "cpp",
            "-o",
            cpp_file.to_str().unwrap(),
        ])
        .output()
        .expect("cpp compile failed");
    assert!(
        cpp_comp.status.success(),
        "[{}] cpp compile failed: {}",
        test_name,
        String::from_utf8_lossy(&cpp_comp.stderr)
    );
    if has_cmd("g++") {
        let cpp_exe = tmp.join(if cfg!(windows) { "app_cpp.exe" } else { "app_cpp" });
        let gpp = Command::new("g++")
            .args([
                "-std=c++20",
                "-O2",
                "-Wall",
                "-Wextra",
                "-Wpedantic",
                cpp_file.to_str().unwrap(),
                "-o",
                cpp_exe.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            gpp.status.success(),
            "[{}] g++ failed:\n{}",
            test_name,
            String::from_utf8_lossy(&gpp.stderr)
        );
        let run_cpp = Command::new(&cpp_exe).output().unwrap();
        assert!(
