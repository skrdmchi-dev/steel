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
#include <sstream>
#include <stdexcept>
#include <string>
#include <unordered_map>
#include <variant>
#include <vector>

namespace steel {

struct Val;
using Arr = std::vector<Val>;
using ArrPtr = std::shared_ptr<Arr>;
using Map = std::unordered_map<std::string, Val>;
using MapPtr = std::shared_ptr<Map>;
using Fn = std::function<Val(Val, const std::vector<Val>&)>;

struct Error : std::runtime_error {
    explicit Error(const std::string& msg) : std::runtime_error(msg) {}
};

struct Val {
    using Storage = std::variant<std::monostate, bool, double, std::string, ArrPtr, MapPtr, Fn>;
    Storage data;

    Val() : data(std::monostate{}) {}
    Val(std::nullptr_t) : data(std::monostate{}) {}
    Val(bool b) : data(b) {}
    Val(int n) : data(static_cast<double>(n)) {}
    Val(double n) : data(n) {}
    Val(const char* s) : data(std::string(s)) {}
    Val(std::string s) : data(std::move(s)) {}
    Val(ArrPtr a) : data(std::move(a)) {}
    Val(MapPtr m) : data(std::move(m)) {}
    Val(Fn f) : data(std::move(f)) {}

    bool is_null() const { return std::holds_alternative<std::monostate>(data); }
    bool is_bool() const { return std::holds_alternative<bool>(data); }
    bool is_num() const { return std::holds_alternative<double>(data); }
    bool is_str() const { return std::holds_alternative<std::string>(data); }
    bool is_arr() const { return std::holds_alternative<ArrPtr>(data); }
    bool is_map() const { return std::holds_alternative<MapPtr>(data); }
    bool is_fn() const { return std::holds_alternative<Fn>(data); }

    double as_num() const {
        if (auto* p = std::get_if<double>(&data)) return *p;
        if (auto* b = std::get_if<bool>(&data)) return *b ? 1.0 : 0.0;
        if (auto* s = std::get_if<std::string>(&data)) {
            try { return std::stod(*s); } catch (...) { return 0.0; }
        }
        return 0.0;
    }

    bool truthy() const {
        if (is_null()) return false;
        if (auto* b = std::get_if<bool>(&data)) return *b;
        if (auto* n = std::get_if<double>(&data)) return *n != 0.0;
        if (auto* s = std::get_if<std::string>(&data)) return !s->empty();
        return true;
    }

    std::string to_string() const {
        if (is_null()) return "null";
        if (auto* b = std::get_if<bool>(&data)) return *b ? "true" : "false";
        if (auto* n = std::get_if<double>(&data)) {
            if (std::floor(*n) == *n && std::abs(*n) < 1e15) {
                return std::to_string(static_cast<long long>(*n));
            }
            std::ostringstream oss;
            oss << *n;
            return oss.str();
        }
        if (auto* s = std::get_if<std::string>(&data)) return *s;
        if (auto* a = std::get_if<ArrPtr>(&data)) {
            std::string out = "[";
            for (size_t i = 0; i < (*a)->size(); ++i) {
                if (i > 0) out += ", ";
                out += (**a)[i].to_string();
            }
            out += "]";
            return out;
        }
        if (is_map()) return "[object]";
        return "[fn]";
    }

    Val operator+(const Val& rhs) const {
        if (is_str() || rhs.is_str()) return Val(to_string() + rhs.to_string());
        return Val(as_num() + rhs.as_num());
    }
    Val operator-(const Val& rhs) const { return Val(as_num() - rhs.as_num()); }
    Val operator*(const Val& rhs) const { return Val(as_num() * rhs.as_num()); }
    Val operator/(const Val& rhs) const { return Val(as_num() / rhs.as_num()); }
    Val operator%(const Val& rhs) const { return Val(std::fmod(as_num(), rhs.as_num())); }
    Val operator-() const { return Val(-as_num()); }

    bool operator==(const Val& rhs) const {
        if (data.index() != rhs.data.index()) return false;
        if (is_null()) return true;
        if (is_bool()) return std::get<bool>(data) == std::get<bool>(rhs.data);
        if (is_num()) return std::get<double>(data) == std::get<double>(rhs.data);
        if (is_str()) return std::get<std::string>(data) == std::get<std::string>(rhs.data);
        if (is_arr()) return std::get<ArrPtr>(data) == std::get<ArrPtr>(rhs.data);
        if (is_map()) return std::get<MapPtr>(data) == std::get<MapPtr>(rhs.data);
        return false;
    }
    bool operator!=(const Val& rhs) const { return !(*this == rhs); }
    bool operator<(const Val& rhs) const { return as_num() < rhs.as_num(); }
    bool operator>(const Val& rhs) const { return as_num() > rhs.as_num(); }
    bool operator<=(const Val& rhs) const { return as_num() <= rhs.as_num(); }
    bool operator>=(const Val& rhs) const { return as_num() >= rhs.as_num(); }

    Val call(const std::vector<Val>& args, const Val& self = Val()) const {
        if (auto* f = std::get_if<Fn>(&data)) return (*f)(self, args);
        return Val();
    }
};

inline Val make_arr(std::initializer_list<Val> items) {
    return Val(std::make_shared<Arr>(items));
}

inline Val make_map(std::initializer_list<std::pair<const std::string, Val>> entries = {}) {
    auto m = std::make_shared<Map>();
    for (const auto& kv : entries) (*m)[kv.first] = kv.second;
    return Val(m);
}

inline Val map_get(const Val& obj, const std::string& key) {
    if (auto* m = std::get_if<MapPtr>(&obj.data)) {
        auto it = (*m)->find(key);
        if (it != (*m)->end()) return it->second;
    }
    return Val();
}

inline void map_set(const Val& obj, const std::string& key, const Val& val) {
    if (auto* m = std::get_if<MapPtr>(&obj.data)) {
        (**m)[key] = val;
    }
}

inline Val call_method(const Val& obj, const std::string& name, const std::vector<Val>& args) {
    Val m = map_get(obj, name);
    return m.call(args, obj);
}

inline Val idx_get(const Val& t, const Val& idx) {
    if (t.is_map() && idx.is_str()) return map_get(t, std::get<std::string>(idx.data));
    if (auto* a = std::get_if<ArrPtr>(&t.data)) {
        long long i = static_cast<long long>(idx.as_num());
        long long n = static_cast<long long>((*a)->size());
        if (i < 0) i += n;
        if (i >= 0 && i < n) return (**a)[static_cast<size_t>(i)];
    }
    return Val();
}

inline void idx_set(const Val& t, const Val& idx, const Val& val) {
    if (t.is_map() && idx.is_str()) {
        map_set(t, std::get<std::string>(idx.data), val);
        return;
    }
    if (auto* a = std::get_if<ArrPtr>(&t.data)) {
        long long i = static_cast<long long>(idx.as_num());
        long long n = static_cast<long long>((*a)->size());
        if (i < 0) i += n;
        if (i >= 0 && i < n) (**a)[static_cast<size_t>(i)] = val;
    }
}

inline Val print_vals(const std::vector<Val>& args) {
    for (size_t i = 0; i < args.size(); ++i) {
        if (i > 0) std::cout << ' ';
        std::cout << args[i].to_string();
    }
    std::cout << '\n';
    return Val();
}

inline Val len_of(const Val& v) {
    if (auto* a = std::get_if<ArrPtr>(&v.data)) return Val(static_cast<double>((*a)->size()));
    if (auto* s = std::get_if<std::string>(&v.data)) return Val(static_cast<double>(s->size()));
    if (auto* m = std::get_if<MapPtr>(&v.data)) return Val(static_cast<double>((*m)->size()));
    return Val(0.0);
}

inline Val push_val(const Val& arr, const Val& item) {
    if (auto* a = std::get_if<ArrPtr>(&arr.data)) (*a)->push_back(item);
