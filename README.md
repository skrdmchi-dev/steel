# Steel

<p align="center">
  <img src="logo.png" width="100" alt="Steel" />
</p>

Steel (`.steel`) is a scripting and compiled programming language written in Rust.

Run `.steel` files directly with the built-in interpreter or stack VM, or compile single files and multi-file projects into C11, C++20, Rust, Go, Python 3, JavaScript, TypeScript, or Lua.

Website & docs: https://steel-lang.pages.dev

## Targets

- **C (C11):** separate `.h`/`.c` modules + `steel_rt.h` (or single file with `--bundle`)
- **C++ (C++20):** RAII `std::variant`/`std::shared_ptr` runtime, native exceptions, lambdas, `#pragma once` headers (compiles cleanly with `-Wall -Wextra -Wpedantic`)
- **Rust:** `mod` files + `steel_rt.rs` + `main.rs`
- **Go:** `package main` modules + `go.mod`
- **Python 3:** separate `.py` modules
- **JavaScript:** CommonJS modules
- **TypeScript:** inferred types (`number`, `string`, `boolean`, typed classes, `export`/`import`, `tsconfig.json`, strict `tsc --noEmit` clean without `@ts-nocheck`)
- **Lua:** Lua 5.4 modules

## Install

### Windows
Download `Steel-v1.0.0-Windows-x64.zip` from [Releases](https://github.com/skrdmchi-dev/steel/releases), extract it, and run `steel-setup.exe`.

### Build from source
```bash
git clone https://github.com/skrdmchi-dev/steel.git
cd steel
cargo build --release
```

## Example

```js
fn boost(x) = x * 2
fn bonus(x) = x + 15

scores = [10, 25, 40]
top = scores.filter(x => x >= 20).map(boost)
best = top[1] |> bonus

print("Top scores:", top)
print("Winner: ${best} pts")
```

## CLI
