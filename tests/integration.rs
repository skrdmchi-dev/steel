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

class Dog extends Animal {
    init(name) {
        this.name = name
    }
    kind() = "dog"
}

class GuideDog extends Dog {
    init(name, owner) {
        this.name = name
        this.owner = owner
    }
    badge() = this.describe() + "->" + this.owner
}

fn parse_csv_totals(raw) {
    lines = split(trim(raw), "\n")
    totals = {}
    for line in lines {
        cols = split(trim(line), ",")
        if len(cols) == 2 {
            dept = cols[0]
            amt = num(cols[1])
            prev = totals[dept] ?? 0
            totals[dept] = prev + amt
        }
    }
    return totals
}

fn bfs_shortest(adj, start, goal) {
    queue = [start]
    dist = {}
    dist[start] = 0
    head = 0
    while head < len(queue) {
        cur = queue[head]
        head += 1
        if cur == goal {
            return dist[cur]
        }
        nbrs = adj[cur] ?? []
        for nxt in nbrs {
            if !contains(dist, nxt) {
                dist[nxt] = dist[cur] + 1
                push(queue, nxt)
            }
        }
    }
    return -1
}

fn make_counter(start) {
    count = start
    return step => {
        count += step
        return count
    }
}

d = new GuideDog("Rex", "Sam")
csv = "eng,120\nsales,80\neng,55\nops,40\nsales,20"
totals = parse_csv_totals(csv)
graph = {
    A: ["B", "C"],
    B: ["D"],
    C: ["D", "E"],
    D: ["F"],
    E: ["F"],
    F: []
}
hops = bfs_shortest(graph, "A", "F")
cnt = make_counter(10)
c1 = cnt(5)
c2 = cnt(3)
print("INHERIT:", d.badge())
print("CSV:", totals["eng"], totals["sales"], totals["ops"])
print("BFS:", hops)
print("CLOSURE:", c1, c2)
"#;

    let steel_file = tmp.join("app.steel");
    fs::write(&steel_file, src).unwrap();

    let run_steel = Command::new(steel_bin())
        .args(["run", steel_file.to_str().unwrap()])
        .output()
        .expect("steel run failed");
    assert!(
        run_steel.status.success(),
        "steel run failed:\n{}",
        String::from_utf8_lossy(&run_steel.stderr)
    );
    let expected = String::from_utf8_lossy(&run_steel.stdout)
        .replace("\r\n", "\n")
        .trim()
        .to_string();
    assert_eq!(
        expected,
        "INHERIT: dog:Rex->Sam\nCSV: 175 100 40\nBFS: 3\nCLOSURE: 15 18"
    );

    // Verify TypeScript: no @ts-nocheck, real types, and identical runtime output
