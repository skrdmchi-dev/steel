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
            run_cpp.status.success(),
            "[{}] cpp binary failed:\n{}",
            test_name,
            String::from_utf8_lossy(&run_cpp.stderr)
        );
        let got = String::from_utf8_lossy(&run_cpp.stdout).replace("\r\n", "\n").trim().to_string();
        assert_eq!(got, expected, "[{}] C++20 output mismatch", test_name);
    }

    // 4. Python 3
    if has_cmd("python") {
        let py_file = tmp.join("app.py");
        let c = Command::new(steel_bin())
            .args(["compile", steel_file.to_str().unwrap(), "--target", "python", "-o", py_file.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(c.status.success());
        let r = Command::new("python").arg(&py_file).output().unwrap();
        assert!(
            r.status.success(),
            "[{}] python failed:\n{}\n--- PY ---\n{}",
            test_name,
            String::from_utf8_lossy(&r.stderr),
            fs::read_to_string(&py_file).unwrap()
        );
        let got = String::from_utf8_lossy(&r.stdout).replace("\r\n", "\n").trim().to_string();
        assert_eq!(got, expected, "[{}] Python output mismatch", test_name);
    }

    // 5. JavaScript
    if has_cmd("node") {
        let js_file = tmp.join("app.js");
        let c = Command::new(steel_bin())
            .args(["compile", steel_file.to_str().unwrap(), "--target", "js", "-o", js_file.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(c.status.success());
        let r = Command::new("node").arg(&js_file).output().unwrap();
        assert!(
            r.status.success(),
            "[{}] js failed:\n{}",
            test_name,
            String::from_utf8_lossy(&r.stderr)
        );
        let got = String::from_utf8_lossy(&r.stdout).replace("\r\n", "\n").trim().to_string();
        assert_eq!(got, expected, "[{}] JS output mismatch", test_name);
    }

    // 6. C11
    if has_cmd("gcc") {
        let c_file = tmp.join("app.c");
        let c = Command::new(steel_bin())
            .args(["compile", steel_file.to_str().unwrap(), "--target", "c", "-o", c_file.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(c.status.success());
        let c_exe = tmp.join(if cfg!(windows) { "app_c.exe" } else { "app_c" });
        let gcc = Command::new("gcc")
            .args([c_file.to_str().unwrap(), "-O2", "-lm", "-o", c_exe.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            gcc.status.success(),
            "[{}] gcc failed:\n{}",
            test_name,
            String::from_utf8_lossy(&gcc.stderr)
        );
        let r = Command::new(&c_exe).output().unwrap();
        assert!(r.status.success(), "[{}] C11 binary failed", test_name);
        let got = String::from_utf8_lossy(&r.stdout).replace("\r\n", "\n").trim().to_string();
        assert_eq!(got, expected, "[{}] C11 output mismatch", test_name);
    }

    // 7. Rust
    if has_cmd("rustc") {
        let rs_file = tmp.join("app.rs");
        let c = Command::new(steel_bin())
            .args(["compile", steel_file.to_str().unwrap(), "--target", "rust", "-o", rs_file.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(c.status.success());
        let rs_exe = tmp.join(if cfg!(windows) { "app_rs.exe" } else { "app_rs" });
        let rc = Command::new("rustc")
            .args([rs_file.to_str().unwrap(), "-A", "warnings", "-o", rs_exe.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            rc.status.success(),
            "[{}] rustc failed:\n{}",
            test_name,
            String::from_utf8_lossy(&rc.stderr)
        );
        let r = Command::new(&rs_exe).output().unwrap();
        assert!(r.status.success(), "[{}] Rust binary failed", test_name);
        let got = String::from_utf8_lossy(&r.stdout).replace("\r\n", "\n").trim().to_string();
        assert_eq!(got, expected, "[{}] Rust output mismatch", test_name);
    }

    // 8. Go
    if Command::new("go").arg("version").output().is_ok() {
        let go_file = tmp.join("app.go");
        let c = Command::new(steel_bin())
            .args(["compile", steel_file.to_str().unwrap(), "--target", "go", "-o", go_file.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(c.status.success());
        let go_exe = tmp.join(if cfg!(windows) { "app_go.exe" } else { "app_go" });
        let gb = Command::new("go")
            .args(["build", "-o", go_exe.to_str().unwrap(), go_file.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            gb.status.success(),
            "[{}] go build failed:\n{}",
            test_name,
            String::from_utf8_lossy(&gb.stderr)
        );
        let r = Command::new(&go_exe).output().unwrap();
        assert!(r.status.success(), "[{}] Go binary failed", test_name);
        let got = String::from_utf8_lossy(&r.stdout).replace("\r\n", "\n").trim().to_string();
        assert_eq!(got, expected, "[{}] Go output mismatch", test_name);
    }

    // 9. Lua 5.4
    if Command::new("lua").arg("-v").output().is_ok() {
        let lua_file = tmp.join("app.lua");
        let c = Command::new(steel_bin())
            .args(["compile", steel_file.to_str().unwrap(), "--target", "lua", "-o", lua_file.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(c.status.success());
        let r = Command::new("lua").arg(&lua_file).output().unwrap();
        assert!(
            r.status.success(),
            "[{}] lua failed:\n{}",
            test_name,
            String::from_utf8_lossy(&r.stderr)
        );
        let got = String::from_utf8_lossy(&r.stdout).replace("\r\n", "\n").trim().to_string();
        assert_eq!(got, expected, "[{}] Lua output mismatch", test_name);
    }

    if Path::new(&tmp).exists() {
        let _ = fs::remove_dir_all(&tmp);
    }
}

#[test]
fn stdlib_parity_across_all_9_runtimes() {
    let io_tmp = std::env::temp_dir()
        .join("steel_stdlib_io_probe.txt")
        .to_string_lossy()
        .replace('\\', "/");

    let src = format!(
        r#"
assert(1 + 1 == 2, "math broke")
print("TYPES:", type(null), type(true), type(42), type("hi"), type([1]), type({{a: 1}}), type(x => x))
print("CONV:", int(-3.9), int("42.8"), bool(0), bool(""), bool("x"), num(" -12.5 "), str(123))

print("STR1:", trim("  hello  "), upper("abC"), lower("AbC"), repeat("ab", 3), repeat("ab", 0))
print("STR2:", pad_start("7", 4, "0"), pad_end("hi", 5, "."), pad_start("long", 2, "0"))
print("STR3:", index_of("banana", "na"), index_of("banana", "zz"), char_code("A"), from_char_code(66))
print("STR4:", starts_with("steel", "st"), ends_with("steel", "el"), replace("a-b-a", "a", "X"), reverse("abc"))
print("STR5:", join(split("x,y,z", ","), "|"), slice("abcdef", 1, 4))

a = [1, 2]
push(a, 3)
p = pop(a)
c = concat(a, [4, 5])
print("ARR1:", p, len(c), join(c, "-"), index_of(c, 4), index_of(c, 99))
print("ARR2:", join(range(0, 7, 2), ","), join(range(6, 0, -2), ","), len(range(5, 2)))
print("ARR3:", sum([1, 2, 3, 4]), avg([2, 4, 6]), join(unique([1, 2, 1, 3, 2]), ","))
print("ARR4:", join(flat([[1, 2], [3], 4]), ","), len(zip([1, 2, 3], [10, 20])), len(enumerate(["a", "b"])))
print("ARR5:", join(sort(["pear", "apple", "banana"]), ","), join(reverse([1, 2, 3]), ","))

nums = [1, 2, 3, 4, 5]
doubled = map(nums, x => x * 2)
evens = filter(nums, x => x % 2 == 0)
tot1 = reduce(nums, (acc, x) => acc + x, 10)
tot2 = reduce(nums, (acc, x) => acc + x)
print("HOF:", join(doubled, ","), join(evens, ","), tot1, tot2, find(nums, x => x > 3), find(nums, x => x > 9), any(nums, x => x == 4), all(nums, x => x > 0), all(nums, x => x > 2))

m = {{a: 1, b: 2}}
print("MAP1:", has_key(m, "a"), has_key(m, "z"), len(m))
d1 = delete_key(m, "a")
d2 = delete_key(m, "a")
print("MAP2:", d1, d2, has_key(m, "a"), len(m))

print("MATH:", abs(-7), floor(2.9), ceil(2.1), round(2.6), sqrt(9), pow(2, 5), min(3, 8), max(3, 8), clamp(15, 0, 10), clamp(-3, 0, 10), sign(-9), sign(0), sign(9))
print("MATH2:", int(sin(PI / 2) * 1000), int(cos(0) * 1000), int(atan2(1, 1) * 1000), int(log(E) * 1000), int(exp(0) * 1000))

obj = {{name: "steel", ok: true, n: 42, arr: [1, null, "x"]}}
raw = json_stringify(obj)
parsed = json_parse(raw)
print("JSON:", parsed["name"], parsed["ok"], parsed["n"], parsed["arr"][0], parsed["arr"][1], parsed["arr"][2])
print("HASH:", hash("steel"), hash(""))

path = "{}"
write_file(path, "hello")
append_file(path, " world")
ex1 = file_exists(path)
txt = read_file(path)
rm = remove_file(path)
ex2 = file_exists(path)
print("IO:", ex1, txt, rm, ex2)
"#,
        io_tmp
    );

    run_all_9_runtimes("stdlib_parity", &src);
}

#[test]
fn weird_inputs_parity_across_all_9_runtimes() {
    let src = r#"
// 1. Weird strings & escapes
e = ""
q = "say \"hi\""
bs = "a\\b\\c"
print("W_STR:", len(e), q, bs, len(split("abc", "")), replace("aaa", "a", ""), replace("abc", "z", "q"))

// 2. Weird numbers, unary chains, null coalesce, short-circuit
print("W_NUM:", -(-5), -(1 + 2) * -3, ((((1 + 2) * 3) - 4) / 2))
print("W_NULL:", null ?? null ?? 0 ?? 99, null ?? false ?? true, null ?? "" ?? "fallback")

side = 0
fn bump() {
    side += 1
    return true
}
sc1 = false && bump()
sc2 = true || bump()
print("W_SC:", sc1, sc2, side)

// 3. Nested ternary & deep closures + non-aliasing factories
t = 1 > 2 ? "a" : (3 < 4 ? "b" : "c")
print("W_TERN:", t)

fn make_adder_factory(base) {
    total = base
    return step => {
        return mult => {
            total += step * mult
            return total
        }
    }
}
f1 = make_adder_factory(10)(2)
f2 = make_adder_factory(100)(5)
print("W_CLOS:", f1(3), f1(4), f2(1), f2(2))

// 4. Mutual recursion
fn is_even(n) {
    if n == 0 { return true }
    return is_odd(n - 1)
}
fn is_odd(n) {
    if n == 0 { return false }
    return is_even(n - 1)
}
print("W_REC:", is_even(10), is_odd(10), is_even(7), is_odd(7))

// 5. Weird control flow: return inside try inside loop, nested try/catch rethrow
fn try_in_loop(limit) {
    i = 0
    while i < limit {
        try {
            if i == 3 {
                return i * 10
            }
            if i == 1 {
                throw "skip1"
            }
        } catch (err) {
            i += 1
            continue
        }
