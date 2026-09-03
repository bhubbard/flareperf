use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;

/// ⚡ Flareperf — Unified Cloudflare Edge Performance & Budget Guardian
///
/// High-performance Rust CLI & library providing Worker bundle size budgets,
/// V8 isolate cold-start analysis, 50-subrequest limit verification, and D1 batching checks.
#[derive(Parser, Debug)]
#[command(
    name = "flareperf",
    author = "Brandon Hubbard <bhubbard@users.noreply.github.com>",
    version = env!("CARGO_PKG_VERSION"),
    about = "⚡ Flareperf — Unified Cloudflare Edge Performance & Budget Guardian",
    long_about = "Flareperf unifies Cloudflare edge performance checks into one fast tool:\n\
- bundle: Enforce gzip/brotli bundle size limits, sourcemap trees, and WASM section budgets\n\
- isolate: Analyze top-level synchronous code complexity and cold-start latency (Error 1101)\n\
- subrequests: Detect unbounded Promise.all fan-outs and 50-subrequest limit violations (Error 1042)\n\
- d1: Detect D1 SQLite N+1 query loops and advise env.DB.batch() optimizations\n\
- check: Run comprehensive pre-deploy performance & budget checks\n\
- completions: Generate shell autocompletions (bash, zsh, fish, powershell)"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 📦 Audit gzip/brotli/zstd bundle sizes against Cloudflare Worker & Pages tier quotas
    Bundle(crate::bundle::cli::CliArgs),

    /// 🧊 Analyze top-level isolate initialization complexity and cold-start latency
    Isolate(IsolateArgs),

    /// 🌐 Detect subrequest fan-out loops and 50-subrequest limit violations
    Subrequests(SubrequestArgs),

    /// 🗄️ Detect D1 SQLite N+1 query loops and suggest env.DB.batch() optimizations
    D1(D1Args),

    /// 🚀 Run complete edge performance & budget check on workspace/bundle
    Check(CheckArgs),

    /// 🐚 Generate shell autocompletion scripts
    Completions {
        /// The target shell
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Args, Debug, Clone)]
pub struct IsolateArgs {
    /// Path to Worker entrypoint or directory
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Maximum allowable top-level complexity score before failing
    #[arg(long, default_value_t = 50)]
    pub max_complexity: u32,

    /// Enforce CI gate check (exit code 1 on failure)
    #[arg(long)]
    pub check: bool,
}

#[derive(Args, Debug, Clone)]
pub struct SubrequestArgs {
    /// Path to Worker source files or directory
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Enforce CI gate check (exit code 1 on failure)
    #[arg(long)]
    pub check: bool,
}

#[derive(Args, Debug, Clone)]
pub struct D1Args {
    /// Path to Worker source files or directory
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Enforce CI gate check (exit code 1 on failure)
    #[arg(long)]
    pub check: bool,
}

#[derive(Args, Debug, Clone)]
pub struct CheckArgs {
    /// Path to project or build output directory (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Fail with exit code 1 if any performance budget is breached
    #[arg(long, default_value_t = true)]
    pub strict: bool,
}
