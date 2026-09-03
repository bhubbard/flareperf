# flareperf

> **Unified Cloudflare Edge Performance & Budget Guardian**  
> Enforce Worker bundle size budgets, analyze V8 isolate cold-start latency, verify 50-subrequest limits, and eliminate D1 SQLite N+1 query bottlenecks.

## 🚀 Quickstart

Run via `npx` (no installation required):

```bash
# Run comprehensive check
npx flareperf check .

# Check bundle size against Cloudflare tier budgets (free, paid, ai)
npx flareperf bundle dist/_worker.js --budget free

# Check V8 isolate cold-start complexity
npx flareperf isolate src/index.ts

# Check for 50-subrequest limit violations
npx flareperf subrequests src/

# Check for D1 N+1 query loops
npx flareperf d1 src/
```

Or install globally:

```bash
npm install -g flareperf
```

## 📦 Rust Crate

Also available on [crates.io](https://crates.io/crates/flareperf):

```bash
cargo install flareperf
```

## 📄 License

MIT © [Brandon Hubbard](https://brandonhubbard.com)
