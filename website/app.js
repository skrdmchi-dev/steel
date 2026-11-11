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
    cmd: "steel compile damage.steel python",
    code: `def calc_damage(base, mult):
    if (mult <= 0):
        return base
    return (base * mult)

hit = calc_damage(24, 1.5)
print("Critical hit:", hit)`
  },
  js: {
    file: "damage.js",
    cmd: "steel compile damage.steel js",
    code: `function calc_damage(base, mult) {
    if (mult <= 0) {
        return base;
    }
    return base * mult;
}

let hit = calc_damage(24, 1.5);
console.log("Critical hit:", hit);`
  },
  ts: {
    file: "damage.ts",
    cmd: "steel compile damage.steel ts",
    code: `function calc_damage(base: number, mult: number): number {
    if ((mult <= 0)) {
        return base;
    }
    return (base * mult);
}

let hit: number = calc_damage(24, 1.5);
_print("Critical hit:", hit);`
  },
  lua: {
    file: "damage.lua",
    cmd: "steel compile damage.steel lua",
    code: `local function calc_damage(base, mult)
    if (mult <= 0) then
        return base
    end
    return (base * mult)
end

local hit = calc_damage(24, 1.5)
print("Critical hit:", hit)`
  }
};

document.addEventListener("DOMContentLoaded", () => {
  const tabs = document.querySelectorAll("[data-lang]");
  const codeEl = document.getElementById("ex-code");
  const fileEl = document.getElementById("ex-file");
  const cmdEl = document.getElementById("ex-cmd");

  if (tabs.length && codeEl) {
    const pick = (key) => {
      const item = EXAMPLES[key];
      if (!item) return;
      tabs.forEach((b) => b.setAttribute("aria-selected", String(b.dataset.lang === key)));
      codeEl.textContent = item.code;
      if (fileEl) fileEl.textContent = item.file;
      if (cmdEl) cmdEl.textContent = item.cmd;
    };
    tabs.forEach((b) => b.addEventListener("click", () => pick(b.dataset.lang)));
    pick("steel");
  }

  const filterInput = document.getElementById("docs-filter");
  const sections = Array.from(document.querySelectorAll(".docs-main section"));
  const tocLinks = Array.from(document.querySelectorAll(".docs-toc a"));

  if (filterInput && sections.length) {
    filterInput.addEventListener("input", () => {
      const q = filterInput.value.trim().toLowerCase();
      sections.forEach((sec) => {
        const match = !q || sec.textContent.toLowerCase().includes(q);
        sec.style.display = match ? "" : "none";
      });
      tocLinks.forEach((a) => {
        const target = document.getElementById(a.getAttribute("href").slice(1));
        a.parentElement.style.display = target && target.style.display !== "none" ? "" : "none";
      });
    });
  }
});
