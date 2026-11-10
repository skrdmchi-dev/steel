const EXAMPLES = {
  steel: {
    file: "damage.steel",
    cmd: "steel run damage.steel",
    code: `fn calc_damage(base, mult) {
    if mult <= 0 {
        return base
    }
    return base * mult
}

hit = calc_damage(24, 1.5)
print("Critical hit:", hit)`
  },
  c: {
    file: "damage.c",
    cmd: "steel compile damage.steel c",
    code: `#include <stdio.h>
#include <stdbool.h>

static double calc_damage(double base, double mult) {
    if (mult <= 0) {
        return base;
    }
    return (base * mult);
}

int main(void) {
    double hit = calc_damage(24, 1.5);
    printf("Critical hit: %g\\n", hit);
    return 0;
}`
  },
  cpp: {
    file: "damage.cpp",
    cmd: "steel compile damage.steel cpp",
    code: `#include "steel_rt.hpp"

steel::Val calc_damage(steel::Val base, steel::Val mult) {
    if ((mult <= steel::Val(0.0)).truthy()) {
        return base;
    }
    return (base * mult);
}

int main() {
    steel::Val hit = calc_damage(steel::Val(24.0), steel::Val(1.5));
    steel::print_vals({steel::Val("Critical hit:"), hit});
    return 0;
}`
  },
  rust: {
    file: "damage.rs",
    cmd: "steel compile damage.steel rust",
    code: `fn calc_damage(base: f64, mult: f64) -> f64 {
    if mult <= 0.0 {
        return base;
    }
    return base * mult;
}

fn main() {
    let hit = calc_damage(24.0, 1.5);
    println!("Critical hit: {}", hit);
}`
  },
  go: {
    file: "damage.go",
    cmd: "steel compile damage.steel go",
    code: `package main

import "fmt"

func calc_damage(base any, mult any) any {
    if __steel_num(mult) <= 0 {
        return base
    }
    return __steel_num(base) * __steel_num(mult)
}

func main() {
    hit := calc_damage(24.0, 1.5)
    fmt.Println("Critical hit:", hit)
}`
  },
  python: {
    file: "damage.py",
