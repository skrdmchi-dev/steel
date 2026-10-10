use crate::ast::{AssignTarget, BinaryOp, Expr, Program, Stmt, UnaryOp};
use crate::codegen::{desugar_class, ModuleUnit};
use std::collections::HashSet;

fn indent(lvl: usize) -> String {
    "    ".repeat(lvl)
}

fn escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}.0", n as i64)
    } else {
        format!("{}", n)
    }
}

fn cpp_safe_ident(name: &str) -> String {
    match name {
        "alignas" | "alignof" | "and" | "and_eq" | "asm" | "auto" | "bitand" | "bitor" | "bool"
        | "break" | "case" | "catch" | "char" | "char8_t" | "char16_t" | "char32_t" | "class"
        | "compl" | "concept" | "const" | "consteval" | "constexpr" | "constinit" | "const_cast"
        | "continue" | "co_await" | "co_return" | "co_yield" | "decltype" | "default" | "delete"
        | "do" | "double" | "dynamic_cast" | "else" | "enum" | "explicit" | "export" | "extern"
        | "false" | "float" | "for" | "friend" | "goto" | "if" | "inline" | "int" | "long"
        | "mutable" | "namespace" | "new" | "noexcept" | "not" | "not_eq" | "nullptr"
        | "operator" | "or" | "or_eq" | "private" | "protected" | "public" | "register"
        | "reinterpret_cast" | "requires" | "return" | "short" | "signed" | "sizeof" | "static"
        | "static_assert" | "static_cast" | "struct" | "switch" | "template" | "this"
        | "thread_local" | "throw" | "true" | "try" | "typedef" | "typeid" | "typename" | "union"
        | "unsigned" | "using" | "virtual" | "void" | "volatile" | "wchar_t" | "while" | "xor"
        | "xor_eq" => format!("st_{}", name),
        _ => name.to_string(),
    }
}

const CPP_RUNTIME_HEADER: &str = r#"#pragma once

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <functional>
#include <cstdint>
#include <iomanip>
#include <iostream>
#include <memory>
