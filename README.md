# ⚡ flareperf

> **Unified Cloudflare Edge Performance & Budget Guardian**  
> Enforce Worker bundle size budgets, analyze V8 isolate cold-start latency, verify 50-subrequest limits, and eliminate D1 SQLite N+1 query bottlenecks.

[![CI](https://github.com/bhubbard/flareperf/actions/workflows/ci.yml/badge.svg)](https://github.com/bhubbard/flareperf/actions)
[![GitHub Pages](https://img.shields.io/badge/docs-bhubbard.github.io%2Fflareperf-black?style=flat-square&logo=github)](https://bhubbard.github.io/flareperf/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)

---

## 🎯 What Problems Does `flareperf` Solve?

Deploying to Cloudflare Workers or Pages requires staying strictly inside edge constraints:
1. **Error 1101 (Worker CPU Time Exceeded / Startup Timeout):** Caused by heavy synchronous code, regex compilations, and static JSON parsing at global module scope.
2. **Error 1042 (Subrequest Limit Exceeded):** Caused by unbatched `fetch()` loops or `Promise.all()` fanouts exceeding Cloudflare's **50 subrequest limit**.
3. **Cold-Start Latency & Bundle Bloat:** Worker bundle sizes exceeding 1MB (Free) or 10MB (Paid) leading to slow startup and edge eviction.
4. **D1 SQLite N+1 Query Bottlenecks:** Executing queries in loops instead of single-roundtrip `env.DB.batch()` transactions.

`flareperf` unifies all 4 budget and performance audits into a single Rust CLI tool.

---

## ⚡ Core Features

- 📦 **`flareperf bundle`**: Enforce gzip/brotli size quotas against Cloudflare Free (1MB), Paid (10MB), and Workers AI (5MB) tiers with sourcemap package decompositions and WASM section budgets.
- 🧊 **`flareperf isolate`**: OXC AST analyzer inspecting global module scope outside request handlers to detect synchronous loops and heavy parsing that trigger Cloudflare Error 1101.
- 🌐 **`flareperf subrequests`**: Detects unbounded `Promise.all` loops, recursive fan-outs, and unbatched edge storage calls that breach Cloudflare's 50 subrequest limit (Error 1042).
- 🗄️ **`flareperf d1`**: Flags SQLite queries inside loops and generates `env.DB.batch([stmt1, stmt2])` refactoring suggestions for single-roundtrip execution.
- 🚀 **`flareperf check`**: Comprehensive pre-deploy CI/CD audit command running all checks.

---

## 🚀 Quickstart

```bash
# Run full end-to-end edge performance check
flareperf check .

# Audit bundle size against Cloudflare tier quotas
flareperf bundle dist/_worker.js --budget free

# Check V8 isolate cold-start complexity
flareperf isolate src/index.ts

# Check for 50-subrequest limit violations
flareperf subrequests src/

# Check for D1 N+1 query loops
flareperf d1 src/

# Generate shell completions
flareperf completions zsh > ~/.zfunc/_flareperf
```

---

## 🤖 GitHub Actions Integration

```yaml
name: Edge Performance & Budget Check
on: [push, pull_request]

jobs:
  performance:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Audit Edge Performance
        uses: bhubbard/flareperf@main
        with:
          command: check
          strict: true
```

---

## 📄 License
MIT © [Brandon Hubbard](https://brandonhubbard.com)
