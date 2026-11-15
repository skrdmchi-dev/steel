use std::fs;
use std::path::Path;
use std::process::Command;

fn steel_bin() -> &'static str {
    env!("CARGO_BIN_EXE_steel")
}

fn has_cmd(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().is_ok()
}

#[test]
fn unit_tests_pass() {
    let out = Command::new(steel_bin())
        .args(["test", "examples/tests.steel"])
        .output()
        .expect("failed to run steel test");
    assert!(
        out.status.success(),
        "steel test failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn vm_runs_factorial() {
    let out = Command::new(steel_bin())
        .args(["vm", "examples/factorial.steel", "--run"])
        .output()
        .expect("failed to run steel vm");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("8! = 40320"), "unexpected vm output: {}", stdout);
}

#[test]
fn inheritance_and_algorithms_parity_across_targets() {
    let tmp = std::env::temp_dir().join("steel_prod_parity_test");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let src = r#"
class Animal {
    init(name) {
        this.name = name
    }
    kind() = "animal"
    describe() = this.kind() + ":" + this.name
}

