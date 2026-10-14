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
    return arr;
}

inline Val pop_val(const Val& arr) {
    if (auto* a = std::get_if<ArrPtr>(&arr.data)) {
        if (!(*a)->empty()) {
            Val v = (*a)->back();
            (*a)->pop_back();
            return v;
        }
    }
    return Val();
}

inline Val range_val(const Val& a, const Val& b, const Val& step_v = Val(1.0)) {
    auto out = std::make_shared<Arr>();
    long long s = static_cast<long long>(a.as_num());
    long long e = static_cast<long long>(b.as_num());
    long long step = static_cast<long long>(step_v.as_num());
    if (step > 0) {
        for (long long i = s; i < e; i += step) out->push_back(Val(static_cast<double>(i)));
    } else if (step < 0) {
        for (long long i = s; i > e; i += step) out->push_back(Val(static_cast<double>(i)));
    }
    return Val(out);
}

inline std::vector<Val> iter_vec(const Val& v) {
    if (auto* a = std::get_if<ArrPtr>(&v.data)) return **a;
    return {};
}

inline Val map_hof(const Val& arr, const Val& fn) {
    auto out = std::make_shared<Arr>();
    for (const auto& item : iter_vec(arr)) out->push_back(fn.call({item}));
    return Val(out);
}

inline Val filter_hof(const Val& arr, const Val& fn) {
    auto out = std::make_shared<Arr>();
    for (const auto& item : iter_vec(arr)) {
        if (fn.call({item}).truthy()) out->push_back(item);
    }
    return Val(out);
}

inline Val reduce_hof(const Val& arr, const Val& fn, const Val& init, bool has_init) {
    auto items = iter_vec(arr);
    size_t start = 0;
    Val acc = init;
    if (!has_init) {
        if (items.empty()) return Val();
        acc = items[0];
        start = 1;
    }
    for (size_t i = start; i < items.size(); ++i) acc = fn.call({acc, items[i]});
    return acc;
}

inline Val find_hof(const Val& arr, const Val& fn) {
    for (const auto& item : iter_vec(arr)) {
        if (fn.call({item}).truthy()) return item;
    }
    return Val();
}

inline Val any_hof(const Val& arr, const Val& fn) {
    for (const auto& item : iter_vec(arr)) {
        if (fn.call({item}).truthy()) return Val(true);
    }
    return Val(false);
}

inline Val all_hof(const Val& arr, const Val& fn) {
    for (const auto& item : iter_vec(arr)) {
        if (!fn.call({item}).truthy()) return Val(false);
    }
    return Val(true);
}

inline Val type_of(const Val& v) {
    if (std::holds_alternative<std::monostate>(v.data)) return Val("null");
    if (std::holds_alternative<bool>(v.data)) return Val("boolean");
    if (std::holds_alternative<double>(v.data)) return Val("number");
    if (std::holds_alternative<std::string>(v.data)) return Val("string");
    if (std::holds_alternative<ArrPtr>(v.data)) return Val("array");
    if (std::holds_alternative<MapPtr>(v.data)) return Val("map");
    return Val("function");
}

inline Val assert_val(const Val& cond, const Val& msg = Val("Assertion failed")) {
    if (!cond.truthy()) throw Error(msg.to_string());
    return Val(true);
}

inline Val has_key_val(const Val& m, const Val& k) {
    if (auto* mp = std::get_if<MapPtr>(&m.data)) return Val((*mp)->count(k.to_string()) > 0);
    return Val(false);
}

inline Val delete_key_val(const Val& m, const Val& k) {
    if (auto* mp = std::get_if<MapPtr>(&m.data)) return Val((*mp)->erase(k.to_string()) > 0);
    return Val(false);
}

inline Val concat_val(const Val& a, const Val& b) {
    auto out = std::make_shared<Arr>(iter_vec(a));
    auto bv = iter_vec(b);
    out->insert(out->end(), bv.begin(), bv.end());
    return Val(out);
}

inline Val index_of_val(const Val& h, const Val& n) {
    if (auto* a = std::get_if<ArrPtr>(&h.data)) {
        for (size_t i = 0; i < (*a)->size(); ++i) {
            if ((**a)[i] == n) return Val(static_cast<double>(i));
        }
        return Val(-1.0);
    }
    std::string hs = h.to_string(), ns = n.to_string();
    size_t pos = hs.find(ns);
    return Val(pos == std::string::npos ? -1.0 : static_cast<double>(pos));
}

inline Val repeat_str(const Val& s, const Val& n) {
    long long c = std::max(0LL, static_cast<long long>(n.as_num()));
    std::string src = s.to_string(), out;
    out.reserve(src.size() * static_cast<size_t>(c));
    for (long long i = 0; i < c; ++i) out += src;
    return Val(std::move(out));
}

inline Val pad_start_str(const Val& s, const Val& w, const Val& ch = Val(" ")) {
    std::string src = s.to_string(), p = ch.to_string();
    size_t width = static_cast<size_t>(std::max(0LL, static_cast<long long>(w.as_num())));
    if (src.size() >= width || p.empty()) return Val(std::move(src));
    size_t need = width - src.size();
    std::string out;
    out.reserve(width);
    for (size_t i = 0; i < need; ++i) out.push_back(p[i % p.size()]);
    out += src;
    return Val(std::move(out));
}

inline Val pad_end_str(const Val& s, const Val& w, const Val& ch = Val(" ")) {
    std::string src = s.to_string(), p = ch.to_string();
    size_t width = static_cast<size_t>(std::max(0LL, static_cast<long long>(w.as_num())));
    if (src.size() >= width || p.empty()) return Val(std::move(src));
    size_t need = width - src.size();
    src.reserve(width);
    for (size_t i = 0; i < need; ++i) src.push_back(p[i % p.size()]);
    return Val(std::move(src));
}

inline Val char_code_val(const Val& s) {
    std::string src = s.to_string();
    return Val(src.empty() ? 0.0 : static_cast<double>(static_cast<unsigned char>(src[0])));
}

inline Val from_char_code_val(const Val& n) {
    int c = std::max(0, static_cast<int>(n.as_num()));
    return Val(std::string(1, static_cast<char>(c)));
}

inline Val sum_val(const Val& arr) {
    double s = 0.0;
    for (const auto& x : iter_vec(arr)) {
        if (auto* d = std::get_if<double>(&x.data)) s += *d;
    }
    return Val(s);
}

inline Val avg_val(const Val& arr) {
    auto items = iter_vec(arr);
    if (items.empty()) return Val(0.0);
    double s = 0.0;
    for (const auto& x : items) {
        if (auto* d = std::get_if<double>(&x.data)) s += *d;
    }
    return Val(s / static_cast<double>(items.size()));
}

inline Val unique_val(const Val& arr) {
    auto out = std::make_shared<Arr>();
    for (const auto& x : iter_vec(arr)) {
        bool dup = false;
        for (const auto& u : *out) if (u == x) { dup = true; break; }
        if (!dup) out->push_back(x);
    }
    return Val(out);
}

inline Val flat_val(const Val& arr) {
    auto out = std::make_shared<Arr>();
    for (const auto& x : iter_vec(arr)) {
        if (auto* inner = std::get_if<ArrPtr>(&x.data)) {
            out->insert(out->end(), (*inner)->begin(), (*inner)->end());
        } else {
            out->push_back(x);
        }
    }
    return Val(out);
}

inline Val zip_val(const Val& a, const Val& b) {
    auto av = iter_vec(a), bv = iter_vec(b);
    size_t n = std::min(av.size(), bv.size());
    auto out = std::make_shared<Arr>();
    for (size_t i = 0; i < n; ++i) {
        auto pair = std::make_shared<Arr>(Arr{av[i], bv[i]});
        out->push_back(Val(pair));
    }
    return Val(out);
}

inline Val enumerate_val(const Val& a) {
    auto av = iter_vec(a);
    auto out = std::make_shared<Arr>();
    for (size_t i = 0; i < av.size(); ++i) {
        auto pair = std::make_shared<Arr>(Arr{Val(static_cast<double>(i)), av[i]});
        out->push_back(Val(pair));
    }
    return Val(out);
}

inline Val hash_val(const Val& v) {
    std::string s = v.to_string();
    std::uint64_t h = 0xcbf29ce484222325ULL;
    for (unsigned char b : s) {
        h ^= static_cast<std::uint64_t>(b);
        h *= 0x0100000001b3ULL;
    }
    std::ostringstream ss;
    ss << std::hex << std::setfill('0') << std::setw(16) << h;
    return Val(ss.str());
}

inline std::string json_escape(const std::string& s) {
    std::string out = "\"";
    for (char c : s) {
        if (c == '\\') out += "\\\\";
        else if (c == '"') out += "\\\"";
        else if (c == '\n') out += "\\n";
        else if (c == '\r') out += "\\r";
        else if (c == '\t') out += "\\t";
        else out.push_back(c);
    }
    out.push_back('"');
    return out;
}

inline std::string json_stringify_raw(const Val& v) {
    if (std::holds_alternative<std::monostate>(v.data)) return "null";
    if (auto* b = std::get_if<bool>(&v.data)) return *b ? "true" : "false";
    if (auto* d = std::get_if<double>(&v.data)) return v.to_string();
    if (auto* s = std::get_if<std::string>(&v.data)) return json_escape(*s);
    if (auto* a = std::get_if<ArrPtr>(&v.data)) {
        std::string out = "[";
        for (size_t i = 0; i < (*a)->size(); ++i) {
            if (i > 0) out += ",";
            out += json_stringify_raw((**a)[i]);
        }
        out += "]";
        return out;
    }
    if (auto* m = std::get_if<MapPtr>(&v.data)) {
        std::string out = "{";
        bool first = true;
        for (const auto& kv : **m) {
            if (!first) out += ",";
            first = false;
            out += json_escape(kv.first) + ":" + json_stringify_raw(kv.second);
        }
        out += "}";
        return out;
    }
    return "null";
}

inline Val json_stringify_val(const Val& v) { return Val(json_stringify_raw(v)); }

inline Val json_parse_at(const std::string& s, size_t& i) {
    auto skip = [&]() { while (i < s.size() && std::isspace(static_cast<unsigned char>(s[i]))) ++i; };
    skip();
    if (i >= s.size()) return Val();
    if (s[i] == '"') {
        ++i;
        std::string out;
        while (i < s.size()) {
            char c = s[i++];
            if (c == '"') break;
            if (c == '\\' && i < s.size()) {
                char e = s[i++];
                if (e == 'n') out.push_back('\n');
                else if (e == 'r') out.push_back('\r');
                else if (e == 't') out.push_back('\t');
                else out.push_back(e);
            } else {
                out.push_back(c);
            }
        }
        return Val(std::move(out));
    }
    if (s[i] == '[') {
        ++i;
        auto arr = std::make_shared<Arr>();
        skip();
        if (i < s.size() && s[i] == ']') { ++i; return Val(arr); }
        while (i < s.size()) {
            arr->push_back(json_parse_at(s, i));
            skip();
            if (i < s.size() && s[i] == ',') { ++i; continue; }
            if (i < s.size() && s[i] == ']') ++i;
            break;
        }
        return Val(arr);
    }
    if (s[i] == '{') {
        ++i;
        auto mp = std::make_shared<Map>();
        skip();
        if (i < s.size() && s[i] == '}') { ++i; return Val(mp); }
        while (i < s.size()) {
            Val k = json_parse_at(s, i);
            skip();
            if (i < s.size() && s[i] == ':') ++i;
            (*mp)[k.to_string()] = json_parse_at(s, i);
            skip();
            if (i < s.size() && s[i] == ',') { ++i; continue; }
            if (i < s.size() && s[i] == '}') ++i;
            break;
        }
        return Val(mp);
    }
    if (s.compare(i, 4, "true") == 0) { i += 4; return Val(true); }
    if (s.compare(i, 5, "false") == 0) { i += 5; return Val(false); }
    if (s.compare(i, 4, "null") == 0) { i += 4; return Val(); }
    size_t st = i;
    while (i < s.size() && (std::isdigit(static_cast<unsigned char>(s[i])) || s[i] == '-' || s[i] == '+' || s[i] == '.' || s[i] == 'e' || s[i] == 'E')) ++i;
    try { return Val(std::stod(s.substr(st, i - st))); } catch (...) { return Val(0.0); }
}

inline Val json_parse_val(const Val& v) {
    std::string s = v.to_string();
    size_t i = 0;
    return json_parse_at(s, i);
}

inline Val append_file_str(const Val& p, const Val& c) {
    std::ofstream f(p.to_string(), std::ios::binary | std::ios::app);
    if (!f) return Val(false);
    f << c.to_string();
    return Val(true);
}

inline Val remove_file_val(const Val& p) {
    return Val(std::filesystem::remove(p.to_string()));
}

inline Val clock_val() {
    using namespace std::chrono;
    auto us = duration_cast<microseconds>(system_clock::now().time_since_epoch()).count();
    return Val(static_cast<double>(us) / 1e6);
}

inline Val upper_str(const Val& v) {
    std::string s = v.to_string();
    for (char& c : s) c = static_cast<char>(std::toupper(static_cast<unsigned char>(c)));
    return Val(std::move(s));
}

inline Val lower_str(const Val& v) {
    std::string s = v.to_string();
    for (char& c : s) c = static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
    return Val(std::move(s));
}

inline Val trim_str(const Val& v) {
    std::string s = v.to_string();
    size_t a = 0;
    while (a < s.size() && std::isspace(static_cast<unsigned char>(s[a]))) ++a;
    size_t b = s.size();
    while (b > a && std::isspace(static_cast<unsigned char>(s[b - 1]))) --b;
    return Val(s.substr(a, b - a));
}

inline Val contains_val(const Val& h, const Val& n) {
    if (auto* m = std::get_if<MapPtr>(&h.data)) return Val((*m)->count(n.to_string()) > 0);
    if (auto* a = std::get_if<ArrPtr>(&h.data)) {
        for (const auto& x : **a) if (x == n) return Val(true);
        return Val(false);
    }
    return Val(h.to_string().find(n.to_string()) != std::string::npos);
}

inline Val starts_with_str(const Val& h, const Val& p) {
    std::string hs = h.to_string(), ps = p.to_string();
    return Val(hs.size() >= ps.size() && hs.compare(0, ps.size(), ps) == 0);
}

inline Val ends_with_str(const Val& h, const Val& p) {
    std::string hs = h.to_string(), ps = p.to_string();
    return Val(hs.size() >= ps.size() && hs.compare(hs.size() - ps.size(), ps.size(), ps) == 0);
}

inline Val split_str(const Val& s, const Val& sep) {
    auto out = std::make_shared<Arr>();
    std::string src = s.to_string(), sp = sep.to_string();
    if (sp.empty()) {
        for (char c : src) out->push_back(Val(std::string(1, c)));
        return Val(out);
    }
    size_t pos = 0;
    while (true) {
        size_t hit = src.find(sp, pos);
        if (hit == std::string::npos) {
            out->push_back(Val(src.substr(pos)));
            break;
        }
        out->push_back(Val(src.substr(pos, hit - pos)));
        pos = hit + sp.size();
    }
    return Val(out);
}

inline Val join_arr(const Val& arr, const Val& sep) {
    std::string sp = sep.to_string(), out;
    auto items = iter_vec(arr);
    for (size_t i = 0; i < items.size(); ++i) {
        if (i > 0) out += sp;
        out += items[i].to_string();
    }
    return Val(std::move(out));
}

inline Val replace_str(const Val& s, const Val& a, const Val& b) {
    std::string src = s.to_string(), old_s = a.to_string(), new_s = b.to_string();
    if (old_s.empty()) return Val(std::move(src));
    size_t pos = 0;
    while ((pos = src.find(old_s, pos)) != std::string::npos) {
        src.replace(pos, old_s.size(), new_s);
        pos += new_s.size();
    }
    return Val(std::move(src));
}

inline Val reverse_val(const Val& v) {
    if (auto* a = std::get_if<ArrPtr>(&v.data)) {
        auto out = std::make_shared<Arr>((*a)->rbegin(), (*a)->rend());
        return Val(out);
    }
    std::string s = v.to_string();
    std::reverse(s.begin(), s.end());
    return Val(std::move(s));
}

inline Val slice_val(const Val& v, const Val& s, const Val& e) {
    long long si = std::max(0LL, static_cast<long long>(s.as_num()));
    if (auto* a = std::get_if<ArrPtr>(&v.data)) {
        long long n = static_cast<long long>((*a)->size());
        long long ei = e.is_null() ? n : std::min(n, static_cast<long long>(e.as_num()));
        if (si > ei) si = ei;
        auto out = std::make_shared<Arr>((*a)->begin() + si, (*a)->begin() + ei);
        return Val(out);
    }
    std::string str = v.to_string();
    long long n = static_cast<long long>(str.size());
    long long ei = e.is_null() ? n : std::min(n, static_cast<long long>(e.as_num()));
    if (si > ei) si = ei;
    return Val(str.substr(static_cast<size_t>(si), static_cast<size_t>(ei - si)));
}

inline Val sort_val(const Val& v) {
    auto out = std::make_shared<Arr>(iter_vec(v));
    std::sort(out->begin(), out->end(), [](const Val& a, const Val& b) {
        return a.to_string() < b.to_string();
    });
    return Val(out);
}

inline Val keys_of(const Val& v) {
    auto out = std::make_shared<Arr>();
    if (auto* m = std::get_if<MapPtr>(&v.data)) {
        for (const auto& kv : **m) out->push_back(Val(kv.first));
    }
    return Val(out);
}

inline Val values_of(const Val& v) {
    auto out = std::make_shared<Arr>();
    if (auto* m = std::get_if<MapPtr>(&v.data)) {
        for (const auto& kv : **m) out->push_back(kv.second);
    }
    return Val(out);
}

inline Val read_file_str(const Val& p) {
    std::ifstream f(p.to_string(), std::ios::binary);
    if (!f) return Val("");
    std::ostringstream ss;
    ss << f.rdbuf();
    return Val(ss.str());
}

inline Val write_file_str(const Val& p, const Val& c) {
    std::ofstream f(p.to_string(), std::ios::binary);
    if (!f) return Val(false);
    f << c.to_string();
    return Val(true);
}

inline Val file_exists_val(const Val& p) {
    return Val(std::filesystem::exists(p.to_string()));
}

inline Val env_val(const Val& k) {
    const char* v = std::getenv(k.to_string().c_str());
    return v ? Val(std::string(v)) : Val();
}

inline Val time_ms_val() {
    using namespace std::chrono;
    auto ms = duration_cast<milliseconds>(system_clock::now().time_since_epoch()).count();
    return Val(static_cast<double>(ms));
}

} // namespace steel
"#;

fn cpp_stmt(stmt: &Stmt, lvl: usize, top_fns: &HashSet<String>, declared: &mut HashSet<String>) -> String {
    let pad = indent(lvl);
    match stmt {
        Stmt::Import(_) => String::new(),
        Stmt::ClassDecl {
            name,
            parent,
            methods,
        } => cpp_stmt(&desugar_class(name, parent, methods), lvl, top_fns, declared),
        Stmt::Let { name, value, .. } => {
            let sn = cpp_safe_ident(name);
            let expr_s = cpp_expr(value, lvl, top_fns);
            if declared.insert(sn.clone()) {
                format!("{}steel::Val {} = {};\n", pad, sn, expr_s)
            } else {
                format!("{}{} = {};\n", pad, sn, expr_s)
            }
        }
        Stmt::Assign { target, value } => match target {
            AssignTarget::Ident(n) => {
                let sn = cpp_safe_ident(n);
                let expr_s = cpp_expr(value, lvl, top_fns);
                if declared.insert(sn.clone()) {
                    format!("{}steel::Val {} = {};\n", pad, sn, expr_s)
                } else {
                    format!("{}{} = {};\n", pad, sn, expr_s)
                }
            }
            AssignTarget::Index { target, index } => format!(
                "{}steel::idx_set({}, {}, {});\n",
                pad,
                cpp_expr(target, lvl, top_fns),
                cpp_expr(index, lvl, top_fns),
                cpp_expr(value, lvl, top_fns)
            ),
            AssignTarget::Member { target, field } => format!(
                "{}steel::map_set({}, \"{}\", {});\n",
                pad,
                cpp_expr(target, lvl, top_fns),
                escape_str(field),
                cpp_expr(value, lvl, top_fns)
            ),
        },
        Stmt::FnDecl { name, params, body } => {
            let sn = cpp_safe_ident(name);
            let mut inner_decl = declared.clone();
            let mut s = format!(
                "{}steel::Val {} = steel::Val(steel::Fn([=](steel::Val st_this, const std::vector<steel::Val>& st_args) mutable -> steel::Val {{\n{}    (void)st_this; (void)st_args;\n",
                pad, sn, pad
            );
            for (i, (p, def)) in params.iter().enumerate() {
                let sp = cpp_safe_ident(p);
                inner_decl.insert(sp.clone());
                let fb = def
                    .as_ref()
                    .map(|d| cpp_expr(d, lvl + 1, top_fns))
                    .unwrap_or_else(|| "steel::Val()".to_string());
                s.push_str(&format!(
                    "{}    steel::Val {} = (st_args.size() > {}) ? st_args[{}] : {};\n",
                    pad, sp, i, i, fb
                ));
            }
            for st in body {
                s.push_str(&cpp_stmt(st, lvl + 1, top_fns, &mut inner_decl));
            }
            s.push_str(&format!("{}    return steel::Val();\n{}}}));\n", pad, pad));
            declared.insert(sn);
            s
        }
        Stmt::TryCatch {
            try_body,
            err_var,
            catch_body,
        } => {
            let mut s = format!("{}try {{\n", pad);
            let mut try_decl = declared.clone();
            for st in try_body {
                s.push_str(&cpp_stmt(st, lvl + 1, top_fns, &mut try_decl));
            }
            let se = cpp_safe_ident(err_var);
            s.push_str(&format!(
                "{}}} catch (const steel::Error& st_err) {{\n{}    steel::Val {} = steel::Val(std::string(st_err.what()));\n{}    (void){};\n",
                pad, pad, se, pad, se
            ));
            let mut catch_decl = declared.clone();
            catch_decl.insert(se);
            for st in catch_body {
                s.push_str(&cpp_stmt(st, lvl + 1, top_fns, &mut catch_decl));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::Throw(e) => format!(
            "{}throw steel::Error(({}).to_string());\n",
            pad,
            cpp_expr(e, lvl, top_fns)
        ),
