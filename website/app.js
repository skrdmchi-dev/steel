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
