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
        Stmt::Test { name, body } => {
            let mut s = format!("{}// test: {}\n{}{{\n", pad, escape_str(name), pad);
            let mut inner = declared.clone();
            for st in body {
                s.push_str(&cpp_stmt(st, lvl + 1, top_fns, &mut inner));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::Defer(inner) => cpp_stmt(inner, lvl, top_fns, declared),
        Stmt::Return(expr) => match expr {
            Some(e) => format!("{}return {};\n", pad, cpp_expr(e, lvl, top_fns)),
            None => format!("{}return steel::Val();\n", pad),
        },
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let mut s = format!("{}if (({}).truthy()) {{\n", pad, cpp_expr(cond, lvl, top_fns));
            let mut t_decl = declared.clone();
            for st in then_branch {
                s.push_str(&cpp_stmt(st, lvl + 1, top_fns, &mut t_decl));
            }
            if let Some(eb) = else_branch {
                s.push_str(&format!("{}}} else {{\n", pad));
                let mut e_decl = declared.clone();
                for st in eb {
                    s.push_str(&cpp_stmt(st, lvl + 1, top_fns, &mut e_decl));
                }
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::While { cond, body } => {
            let mut s = format!(
                "{}while (({}).truthy()) {{\n",
                pad,
                cpp_expr(cond, lvl, top_fns)
            );
            let mut inner = declared.clone();
            for st in body {
                s.push_str(&cpp_stmt(st, lvl + 1, top_fns, &mut inner));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::For {
            var,
            iterable,
            body,
        } => {
            let sv = cpp_safe_ident(var);
            let mut inner = declared.clone();
            inner.insert(sv.clone());
            if let Expr::Range { start, end } = iterable {
                let mut s = format!(
                    "{}for (double st_i_{} = ({}).as_num(), st_end_{} = ({}).as_num(); st_i_{} < st_end_{}; st_i_{} += 1.0) {{\n{}    steel::Val {}(st_i_{});\n{}    (void){};\n",
                    pad,
                    sv,
                    cpp_expr(start, lvl, top_fns),
                    sv,
                    cpp_expr(end, lvl, top_fns),
                    sv,
                    sv,
                    sv,
                    pad,
                    sv,
                    sv,
                    pad,
                    sv
                );
                for st in body {
                    s.push_str(&cpp_stmt(st, lvl + 1, top_fns, &mut inner));
                }
                s.push_str(&format!("{}}}\n", pad));
                return s;
            }
            let mut s = format!(
                "{}for (steel::Val {} : steel::iter_vec({})) {{\n{}    (void){};\n",
                pad,
                sv,
                cpp_expr(iterable, lvl, top_fns),
                pad,
                sv
            );
            for st in body {
                s.push_str(&cpp_stmt(st, lvl + 1, top_fns, &mut inner));
            }
            s.push_str(&format!("{}}}\n", pad));
            s
        }
        Stmt::Break => format!("{}break;\n", pad),
        Stmt::Continue => format!("{}continue;\n", pad),
        Stmt::Expr(e) => format!("{}(void)({});\n", pad, cpp_expr(e, lvl, top_fns)),
    }
}

fn cpp_expr(expr: &Expr, lvl: usize, top_fns: &HashSet<String>) -> String {
    match expr {
        Expr::Number(n) => format!("steel::Val({})", fmt_num(*n)),
        Expr::String(s) => format!("steel::Val(\"{}\")", escape_str(s)),
        Expr::Bool(b) => format!("steel::Val({})", b),
        Expr::Null => "steel::Val()".to_string(),
        Expr::Ident(name) => match name.as_str() {
            "PI" => "steel::Val(3.14159265358979323846)".to_string(),
            "E" => "steel::Val(2.71828182845904523536)".to_string(),
            _ => {
                let sn = cpp_safe_ident(name);
                if top_fns.contains(name) {
                    format!("steel::Val(steel::Fn(st_fn_{}))", sn)
                } else {
                    sn
                }
            }
        },
        Expr::Array(items) => {
            let inner: Vec<String> = items.iter().map(|e| cpp_expr(e, lvl, top_fns)).collect();
            format!("steel::make_arr({{{}}})", inner.join(", "))
        }
        Expr::Map(entries) => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{{\"{}\", {}}}", escape_str(k), cpp_expr(v, lvl, top_fns)))
                .collect();
            format!("steel::make_map({{{}}})", inner.join(", "))
        }
        Expr::New { class_name, args } => {
            let arg_strs: Vec<String> = args.iter().map(|a| cpp_expr(a, lvl, top_fns)).collect();
            format!("{}({})", cpp_safe_ident(class_name), arg_strs.join(", "))
        }
        Expr::Ternary {
            cond,
            then_expr,
            else_expr,
        } => format!(
            "(({}).truthy() ? ({}) : ({}))",
            cpp_expr(cond, lvl, top_fns),
            cpp_expr(then_expr, lvl, top_fns),
            cpp_expr(else_expr, lvl, top_fns)
        ),
        Expr::Binary { left, op, right } => {
            let l = cpp_expr(left, lvl, top_fns);
            let r = cpp_expr(right, lvl, top_fns);
            match op {
                BinaryOp::Add => format!("({} + {})", l, r),
                BinaryOp::Sub => format!("({} - {})", l, r),
                BinaryOp::Mul => format!("({} * {})", l, r),
                BinaryOp::Div => format!("({} / {})", l, r),
                BinaryOp::Mod => format!("({} % {})", l, r),
                BinaryOp::Pow => format!("steel::Val(std::pow(({}).as_num(), ({}).as_num()))", l, r),
                BinaryOp::Eq => format!("steel::Val({} == {})", l, r),
                BinaryOp::Neq => format!("steel::Val({} != {})", l, r),
                BinaryOp::Lt => format!("steel::Val({} < {})", l, r),
                BinaryOp::Gt => format!("steel::Val({} > {})", l, r),
                BinaryOp::Lte => format!("steel::Val({} <= {})", l, r),
                BinaryOp::Gte => format!("steel::Val({} >= {})", l, r),
                BinaryOp::And => format!("steel::Val(({}).truthy() && ({}).truthy())", l, r),
                BinaryOp::Or => format!("steel::Val(({}).truthy() || ({}).truthy())", l, r),
                BinaryOp::BitAnd => format!(
                    "steel::Val(static_cast<double>(static_cast<long long>(({}).as_num()) & static_cast<long long>(({}).as_num())))",
                    l, r
                ),
                BinaryOp::BitOr => format!(
                    "steel::Val(static_cast<double>(static_cast<long long>(({}).as_num()) | static_cast<long long>(({}).as_num())))",
                    l, r
                ),
                BinaryOp::BitXor => format!(
                    "steel::Val(static_cast<double>(static_cast<long long>(({}).as_num()) ^ static_cast<long long>(({}).as_num())))",
                    l, r
                ),
                BinaryOp::Shl => format!(
                    "steel::Val(static_cast<double>(static_cast<long long>(({}).as_num()) << static_cast<long long>(({}).as_num())))",
                    l, r
                ),
                BinaryOp::Shr => format!(
                    "steel::Val(static_cast<double>(static_cast<long long>(({}).as_num()) >> static_cast<long long>(({}).as_num())))",
                    l, r
                ),
                BinaryOp::NullCoalesce => format!(
                    "([&]() -> steel::Val {{ steel::Val st_l = {}; return st_l.is_null() ? ({}) : st_l; }})()",
                    l, r
                ),
            }
        }
        Expr::Unary { op, expr } => {
            let v = cpp_expr(expr, lvl, top_fns);
            match op {
                UnaryOp::Neg => format!("(-({}))", v),
                UnaryOp::Not => format!("steel::Val(!({}).truthy())", v),
                UnaryOp::BitNot => format!(
                    "steel::Val(static_cast<double>(~static_cast<long long>(({}).as_num())))",
                    v
                ),
            }
        }
        Expr::Call { callee, args } => {
            let arg_strs: Vec<String> = args.iter().map(|a| cpp_expr(a, lvl, top_fns)).collect();
            if let Expr::Ident(name) = &**callee {
                match name.as_str() {
                    "print" | "println" => {
                        return format!("steel::print_vals({{{}}})", arg_strs.join(", "));
                    }
                    "str" if args.len() == 1 => {
                        return format!("steel::Val(({}).to_string())", arg_strs[0]);
                    }
                    "num" if args.len() == 1 => {
                        return format!("steel::Val(({}).as_num())", arg_strs[0]);
                    }
                    "len" if args.len() == 1 => return format!("steel::len_of({})", arg_strs[0]),
                    "push" if args.len() == 2 => {
                        return format!("steel::push_val({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "pop" if args.len() == 1 => return format!("steel::pop_val({})", arg_strs[0]),
                    "range" if args.len() >= 2 => {
                        let st = arg_strs.get(2).cloned().unwrap_or_else(|| "steel::Val(1.0)".to_string());
                        return format!("steel::range_val({}, {}, {})", arg_strs[0], arg_strs[1], st);
                    }
                    "map" if args.len() == 2 => {
                        return format!("steel::map_hof({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "filter" if args.len() == 2 => {
                        return format!("steel::filter_hof({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "reduce" if args.len() >= 2 => {
                        let has_init = args.len() >= 3;
                        let init = arg_strs.get(2).cloned().unwrap_or_else(|| "steel::Val()".to_string());
                        return format!("steel::reduce_hof({}, {}, {}, {})", arg_strs[0], arg_strs[1], init, has_init);
                    }
                    "find" if args.len() == 2 => {
                        return format!("steel::find_hof({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "any" if args.len() == 2 => {
                        return format!("steel::any_hof({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "all" if args.len() == 2 => {
                        return format!("steel::all_hof({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "int" if args.len() == 1 => {
                        return format!("steel::Val(std::trunc(({}).as_num()))", arg_strs[0]);
                    }
                    "bool" if args.len() == 1 => {
                        return format!("steel::Val(({}).truthy())", arg_strs[0]);
                    }
                    "type" if args.len() == 1 => {
                        return format!("steel::type_of({})", arg_strs[0]);
                    }
                    "assert" if !args.is_empty() => {
                        if args.len() >= 2 {
                            return format!("steel::assert_val({}, {})", arg_strs[0], arg_strs[1]);
                        }
                        return format!("steel::assert_val({})", arg_strs[0]);
                    }
                    "has_key" if args.len() == 2 => {
                        return format!("steel::has_key_val({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "delete_key" if args.len() == 2 => {
                        return format!("steel::delete_key_val({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "concat" if args.len() == 2 => {
                        return format!("steel::concat_val({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "index_of" if args.len() == 2 => {
                        return format!("steel::index_of_val({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "repeat" if args.len() == 2 => {
                        return format!("steel::repeat_str({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "pad_start" if args.len() >= 2 => {
                        let ch = arg_strs.get(2).cloned().unwrap_or_else(|| "steel::Val(\" \")".to_string());
                        return format!("steel::pad_start_str({}, {}, {})", arg_strs[0], arg_strs[1], ch);
                    }
                    "pad_end" if args.len() >= 2 => {
                        let ch = arg_strs.get(2).cloned().unwrap_or_else(|| "steel::Val(\" \")".to_string());
                        return format!("steel::pad_end_str({}, {}, {})", arg_strs[0], arg_strs[1], ch);
                    }
                    "char_code" if args.len() == 1 => {
                        return format!("steel::char_code_val({})", arg_strs[0]);
                    }
                    "from_char_code" if args.len() == 1 => {
                        return format!("steel::from_char_code_val({})", arg_strs[0]);
                    }
                    "sum" if args.len() == 1 => return format!("steel::sum_val({})", arg_strs[0]),
                    "avg" if args.len() == 1 => return format!("steel::avg_val({})", arg_strs[0]),
                    "unique" if args.len() == 1 => return format!("steel::unique_val({})", arg_strs[0]),
                    "flat" if args.len() == 1 => return format!("steel::flat_val({})", arg_strs[0]),
                    "zip" if args.len() == 2 => return format!("steel::zip_val({}, {})", arg_strs[0], arg_strs[1]),
                    "enumerate" if args.len() == 1 => return format!("steel::enumerate_val({})", arg_strs[0]),
                    "clamp" if args.len() == 3 => {
                        return format!("steel::Val(std::fmin(std::fmax(({}).as_num(), ({}).as_num()), ({}).as_num()))", arg_strs[0], arg_strs[1], arg_strs[2]);
                    }
                    "sign" if args.len() == 1 => {
                        return format!("steel::Val((({}).as_num() > 0.0) ? 1.0 : ((({}).as_num() < 0.0) ? -1.0 : 0.0))", arg_strs[0], arg_strs[0]);
                    }
                    "atan2" if args.len() == 2 => {
                        return format!("steel::Val(std::atan2(({}).as_num(), ({}).as_num()))", arg_strs[0], arg_strs[1]);
                    }
                    "log" | "exp" if args.len() == 1 => {
                        return format!("steel::Val(std::{}(({}).as_num()))", name, arg_strs[0]);
                    }
                    "json_stringify" if args.len() == 1 => {
                        return format!("steel::json_stringify_val({})", arg_strs[0]);
                    }
                    "json_parse" if args.len() == 1 => {
                        return format!("steel::json_parse_val({})", arg_strs[0]);
                    }
                    "hash" if args.len() == 1 => {
                        return format!("steel::hash_val({})", arg_strs[0]);
                    }
                    "append_file" if args.len() == 2 => {
                        return format!("steel::append_file_str({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "remove_file" if args.len() == 1 => {
                        return format!("steel::remove_file_val({})", arg_strs[0]);
                    }
                    "clock" if args.is_empty() => return "steel::clock_val()".to_string(),
                    "abs" if args.len() == 1 => {
                        return format!("steel::Val(std::fabs(({}).as_num()))", arg_strs[0]);
                    }
                    "sqrt" | "sin" | "cos" | "tan" | "floor" | "ceil" | "round" if args.len() == 1 => {
                        return format!("steel::Val(std::{} (({}).as_num()))", name, arg_strs[0]);
                    }
                    "pow" if args.len() == 2 => {
                        return format!("steel::Val(std::pow(({}).as_num(), ({}).as_num()))", arg_strs[0], arg_strs[1]);
                    }
                    "min" if args.len() == 2 => {
                        return format!("steel::Val(std::fmin(({}).as_num(), ({}).as_num()))", arg_strs[0], arg_strs[1]);
                    }
                    "max" if args.len() == 2 => {
                        return format!("steel::Val(std::fmax(({}).as_num(), ({}).as_num()))", arg_strs[0], arg_strs[1]);
                    }
                    "upper" if args.len() == 1 => return format!("steel::upper_str({})", arg_strs[0]),
                    "lower" if args.len() == 1 => return format!("steel::lower_str({})", arg_strs[0]),
                    "trim" if args.len() == 1 => return format!("steel::trim_str({})", arg_strs[0]),
                    "contains" if args.len() == 2 => {
                        return format!("steel::contains_val({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "starts_with" if args.len() == 2 => {
                        return format!("steel::starts_with_str({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "ends_with" if args.len() == 2 => {
                        return format!("steel::ends_with_str({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "split" if args.len() == 2 => {
                        return format!("steel::split_str({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "join" if args.len() == 2 => {
                        return format!("steel::join_arr({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "replace" if args.len() == 3 => {
                        return format!("steel::replace_str({}, {}, {})", arg_strs[0], arg_strs[1], arg_strs[2]);
                    }
                    "reverse" if args.len() == 1 => return format!("steel::reverse_val({})", arg_strs[0]),
                    "slice" if args.len() >= 2 => {
                        let e = arg_strs.get(2).cloned().unwrap_or_else(|| "steel::Val()".to_string());
                        return format!("steel::slice_val({}, {}, {})", arg_strs[0], arg_strs[1], e);
                    }
                    "sort" if args.len() == 1 => return format!("steel::sort_val({})", arg_strs[0]),
                    "keys" if args.len() == 1 => return format!("steel::keys_of({})", arg_strs[0]),
                    "values" if args.len() == 1 => return format!("steel::values_of({})", arg_strs[0]),
                    "read_file" if args.len() == 1 => return format!("steel::read_file_str({})", arg_strs[0]),
                    "write_file" if args.len() == 2 => {
                        return format!("steel::write_file_str({}, {})", arg_strs[0], arg_strs[1]);
                    }
                    "file_exists" if args.len() == 1 => return format!("steel::file_exists_val({})", arg_strs[0]),
                    "env" if args.len() == 1 => return format!("steel::env_val({})", arg_strs[0]),
                    "time_ms" if args.is_empty() => return "steel::time_ms_val()".to_string(),
                    _ => {
                        if top_fns.contains(name) {
                            return format!("{}({})", cpp_safe_ident(name), arg_strs.join(", "));
                        }
                    }
                }
            }
            if let Expr::Member { target, field } = &**callee {
                return format!(
                    "steel::call_method({}, \"{}\", {{{}}})",
                    cpp_expr(target, lvl, top_fns),
                    escape_str(field),
                    arg_strs.join(", ")
                );
            }
            format!(
                "({}).call({{{}}})",
                cpp_expr(callee, lvl, top_fns),
                arg_strs.join(", ")
            )
        }
        Expr::Index { target, index } => format!(
            "steel::idx_get({}, {})",
            cpp_expr(target, lvl, top_fns),
            cpp_expr(index, lvl, top_fns)
        ),
        Expr::Member { target, field } => format!(
            "steel::map_get({}, \"{}\")",
            cpp_expr(target, lvl, top_fns),
            escape_str(field)
        ),
        Expr::FnExpr { params, body } => {
            let mut inner_decl = HashSet::new();
            inner_decl.insert("st_this".to_string());
            let mut s = String::from(
                "steel::Val(steel::Fn([=](steel::Val st_this, const std::vector<steel::Val>& st_args) mutable -> steel::Val {\n",
            );
            s.push_str(&format!("{}    (void)st_this; (void)st_args;\n", indent(lvl)));
            for (i, (p, def)) in params.iter().enumerate() {
                let sp = cpp_safe_ident(p);
                inner_decl.insert(sp.clone());
                let fb = def
                    .as_ref()
                    .map(|d| cpp_expr(d, lvl + 1, top_fns))
                    .unwrap_or_else(|| "steel::Val()".to_string());
                s.push_str(&format!(
                    "{}    steel::Val {} = (st_args.size() > {}) ? st_args[{}] : {};\n",
                    indent(lvl),
                    sp,
                    i,
                    i,
                    fb
                ));
            }
            for st in body {
                s.push_str(&cpp_stmt(st, lvl + 1, top_fns, &mut inner_decl));
            }
            s.push_str(&format!("{}    return steel::Val();\n{}}}))", indent(lvl), indent(lvl)));
            s
        }
        Expr::Range { start, end } => format!(
            "steel::range_val({}, {})",
            cpp_expr(start, lvl, top_fns),
            cpp_expr(end, lvl, top_fns)
        ),
        Expr::Spread(inner) => cpp_expr(inner, lvl, top_fns),
    }
}

fn cpp_top_fn(
    name: &str,
    params: &[(String, Option<Expr>)],
    body: &[Stmt],
    top_fns: &HashSet<String>,
) -> String {
    let sn = cpp_safe_ident(name);
    let mut declared = HashSet::new();
    let sig: Vec<String> = params
        .iter()
        .map(|(p, _)| {
            let sp = cpp_safe_ident(p);
            declared.insert(sp.clone());
            format!("steel::Val {}", sp)
        })
        .collect();
    let mut s = format!("steel::Val {}({}) {{\n", sn, sig.join(", "));
    for st in body {
        s.push_str(&cpp_stmt(st, 1, top_fns, &mut declared));
    }
    if !matches!(body.last(), Some(Stmt::Return(_))) {
        s.push_str("    return steel::Val();\n");
    }
    s.push_str("}\n");
    let wrap_args: Vec<String> = (0..params.len())
        .map(|i| format!("(st_args.size() > {} ? st_args[{}] : steel::Val())", i, i))
        .collect();
    s.push_str(&format!(
        "steel::Val st_fn_{}(steel::Val, const std::vector<steel::Val>& st_args) {{ (void)st_args; return {}({}); }}\n\n",
        sn,
        sn,
        wrap_args.join(", ")
    ));
    s
}

pub fn emit_cpp_single(program: &Program) -> String {
    let mut top_fns = HashSet::new();
    let mut protos = String::new();
    for stmt in &program.statements {
        match stmt {
            Stmt::FnDecl { name, params, .. } => {
                top_fns.insert(name.clone());
                let sn = cpp_safe_ident(name);
                let sig: Vec<String> = params.iter().map(|_| "steel::Val".to_string()).collect();
                protos.push_str(&format!("steel::Val {}({});\n", sn, sig.join(", ")));
                protos.push_str(&format!(
                    "steel::Val st_fn_{}(steel::Val, const std::vector<steel::Val>&);\n",
                    sn
                ));
            }
            Stmt::ClassDecl {
                name,
                parent,
                methods,
            } => {
                top_fns.insert(name.clone());
                if let Stmt::FnDecl { name, params, .. } = desugar_class(name, parent, methods) {
                    let sn = cpp_safe_ident(&name);
                    let sig: Vec<String> = params.iter().map(|_| "steel::Val".to_string()).collect();
                    protos.push_str(&format!("steel::Val {}({});\n", sn, sig.join(", ")));
                    protos.push_str(&format!(
                        "steel::Val st_fn_{}(steel::Val, const std::vector<steel::Val>&);\n",
                        sn
                    ));
                }
            }
            _ => {}
        }
    }

    let mut fns_out = String::new();
    let mut main_out = String::new();
    let mut declared = HashSet::new();
    for stmt in &program.statements {
        match stmt {
            Stmt::Import(_) => {}
            Stmt::FnDecl { name, params, body } => {
                fns_out.push_str(&cpp_top_fn(name, params, body, &top_fns));
            }
            Stmt::ClassDecl {
                name,
                parent,
                methods,
            } => {
                if let Stmt::FnDecl { name, params, body } = desugar_class(name, parent, methods) {
                    fns_out.push_str(&cpp_top_fn(&name, &params, &body, &top_fns));
                }
            }
            other => main_out.push_str(&cpp_stmt(other, 1, &top_fns, &mut declared)),
        }
    }

    let mut out = String::from(CPP_RUNTIME_HEADER);
    out.push('\n');
    if !protos.is_empty() {
        out.push_str(&protos);
        out.push('\n');
    }
    out.push_str(&fns_out);
    out.push_str("int main() {\n");
    out.push_str(&main_out);
    out.push_str("    return 0;\n}\n");
    out
}

pub fn emit_cpp_project(modules: &[ModuleUnit]) -> Vec<(String, String)> {
    let mut top_fns = HashSet::new();
    for m in modules {
        for stmt in &m.program.statements {
            match stmt {
                Stmt::FnDecl { name, .. } | Stmt::ClassDecl { name, .. } => {
                    top_fns.insert(name.clone());
                }
                _ => {}
            }
        }
    }

    let mut files = Vec::new();
    files.push(("steel_rt.hpp".to_string(), CPP_RUNTIME_HEADER.to_string()));

    for m in modules {
        let mut hdr = String::from("#pragma once\n\n#include \"steel_rt.hpp\"\n\n");
        let mut fns_out = String::new();
        let mut main_out = String::new();
        let mut declared = HashSet::new();

        for stmt in &m.program.statements {
            match stmt {
                Stmt::Import(_) => {}
                Stmt::FnDecl { name, params, body } => {
                    let sn = cpp_safe_ident(name);
                    let sig: Vec<String> = params.iter().map(|_| "steel::Val".to_string()).collect();
                    hdr.push_str(&format!("steel::Val {}({});\n", sn, sig.join(", ")));
                    hdr.push_str(&format!(
                        "steel::Val st_fn_{}(steel::Val, const std::vector<steel::Val>&);\n",
                        sn
                    ));
                    fns_out.push_str(&cpp_top_fn(name, params, body, &top_fns));
                }
                Stmt::ClassDecl {
                    name,
                    parent,
                    methods,
                } => {
                    if let Stmt::FnDecl { name, params, body } = desugar_class(name, parent, methods)
                    {
                        let sn = cpp_safe_ident(&name);
                        let sig: Vec<String> =
                            params.iter().map(|_| "steel::Val".to_string()).collect();
                        hdr.push_str(&format!("steel::Val {}({});\n", sn, sig.join(", ")));
                        hdr.push_str(&format!(
                            "steel::Val st_fn_{}(steel::Val, const std::vector<steel::Val>&);\n",
                            sn
                        ));
                        fns_out.push_str(&cpp_top_fn(&name, &params, &body, &top_fns));
                    }
                }
                other => main_out.push_str(&cpp_stmt(other, 1, &top_fns, &mut declared)),
            }
        }

        if !m.is_entry {
            files.push((format!("{}.hpp", m.name), hdr));
        }

        let mut src = String::new();
        if m.is_entry {
            src.push_str("#include \"steel_rt.hpp\"\n");
        } else {
            src.push_str(&format!("#include \"{}.hpp\"\n", m.name));
        }
        let mut included = HashSet::new();
        for imp in &m.imports {
            if imp != &m.name && included.insert(imp.clone()) {
                src.push_str(&format!("#include \"{}.hpp\"\n", imp));
            }
        }
        src.push('\n');
        src.push_str(&fns_out);
        if m.is_entry {
            src.push_str("int main() {\n");
            src.push_str(&main_out);
            src.push_str("    return 0;\n}\n");
        }
        files.push((format!("{}.cpp", m.name), src));
    }

    files
}
