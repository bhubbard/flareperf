use crate::bundle::budget::{parse_size_str, BudgetConfig, TierPreset};
use crate::bundle::compress::CompressionAlgo;
use crate::bundle::report::OutputFormat;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "edge-bundle-budget",
    about = "Audit gzip/brotli/zstd bundle sizes against Cloudflare Worker & Pages tier quotas, inspect sourcemaps, and decompose WebAssembly binaries",
    version
)]
pub struct CliArgs {
    /// Path to Worker script, WASM file, or build directory (e.g. dist/, .wrangler/)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Cloudflare tier quota preset [free (1MB), paid (10MB), workers-ai (5MB), pages-asset (25MB), custom, none]
    #[arg(short = 'b', long = "budget", default_value = "free")]
    pub budget: TierPreset,

    /// Custom max uncompressed/raw size threshold (e.g. '500KB', '2MB', '1048576')
    #[arg(long = "max-size")]
    pub max_size: Option<String>,

    /// Custom max gzip size threshold (e.g. '500KB', '1MB')
    #[arg(long = "max-gzip")]
    pub max_gzip: Option<String>,

    /// Custom max brotli size threshold (e.g. '400KB')
    #[arg(long = "max-brotli")]
    pub max_brotli: Option<String>,

    /// Warning threshold percentage before breach (default: 80.0%)
    #[arg(long = "warn-threshold", default_value_t = 80.0)]
    pub warn_threshold: f64,

    /// Primary compression algorithm used for quota evaluation [gzip, brotli, zstd, raw]
    #[arg(short = 'c', long = "compression", default_value = "gzip")]
    pub compression: CompressionAlgo,

    /// Enable / disable JavaScript sourcemap module breakdown
    #[arg(long = "sourcemap", default_value_t = true, action = clap::ArgAction::Set)]
    pub sourcemap: bool,

    /// Enable / disable WebAssembly binary section decomposition
    #[arg(long = "wasm", default_value_t = true, action = clap::ArgAction::Set)]
    pub wasm: bool,

    /// Number of top npm packages to display in sourcemap breakdown
    #[arg(long = "top-packages", default_value_t = 10)]
    pub top_packages: usize,

    /// Report output format [terminal, json, markdown, sarif]
    #[arg(short = 'f', long = "format", default_value = "terminal")]
    pub format: OutputFormat,

    /// Output file path to write results into (defaults to stdout)
    #[arg(short = 'o', long = "output")]
    pub output: Option<PathBuf>,

    /// Enforce budget check: exit code 1 if budget is breached
    #[arg(long = "check")]
    pub check: bool,

    /// Disable colored terminal output
    #[arg(long = "no-color")]
    pub no_color: bool,

    /// Enable verbose diagnostic logging
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,
}

impl CliArgs {
    pub fn build_budget_config(&self) -> Result<BudgetConfig, String> {
        let mut compression = self.compression;
        let mut limit_bytes = self.budget.default_limit_bytes().unwrap_or(0);
        let mut preset = self.budget;

        if let Some(ref custom_str) = self.max_gzip {
            limit_bytes = parse_size_str(custom_str)?;
            compression = CompressionAlgo::Gzip;
            preset = TierPreset::Custom;
        } else if let Some(ref custom_str) = self.max_brotli {
            limit_bytes = parse_size_str(custom_str)?;
            compression = CompressionAlgo::Brotli;
            preset = TierPreset::Custom;
        } else if let Some(ref custom_str) = self.max_size {
            limit_bytes = parse_size_str(custom_str)?;
            preset = TierPreset::Custom;
        }

        Ok(BudgetConfig {
            preset,
            limit_bytes,
            compression,
            warn_threshold_pct: self.warn_threshold,
        })
    }
}
