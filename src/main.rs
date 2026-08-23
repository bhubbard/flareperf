use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use colored::Colorize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::exit;
use walkdir::WalkDir;

use flareperf::cli::{CheckArgs, Cli, Commands, D1Args, IsolateArgs, SubrequestArgs};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Bundle(args) => {
            run_bundle_command(args)?;
        }
        Commands::Isolate(args) => {
            run_isolate_command(args)?;
        }
        Commands::Subrequests(args) => {
            run_subrequests_command(args)?;
        }
        Commands::D1(args) => {
            run_d1_command(args)?;
        }
        Commands::Check(args) => {
            run_check_command(args)?;
        }
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            generate(shell, &mut cmd, bin_name, &mut io::stdout());
        }
    }

    Ok(())
}

fn run_bundle_command(args: flareperf::bundle::cli::CliArgs) -> Result<()> {
    let budget_config = args.build_budget_config().map_err(|e| anyhow::anyhow!(e))?;
    let use_color = !args.no_color && std::env::var("NO_COLOR").is_err();

    let options = flareperf::bundle::analyzer::AnalyzerOptions {
        target_path: args.path.clone(),
        budget_config,
        inspect_sourcemaps: args.sourcemap,
        inspect_wasm: args.wasm,
        top_packages: args.top_packages,
    };

    let report = flareperf::bundle::analyzer::analyze_target(&options)
        .map_err(|e| anyhow::anyhow!("Analysis error: {}", e))?;

    let output_str = flareperf::bundle::report::render_report(&report, args.format, use_color)
        .map_err(|e| anyhow::anyhow!("Report error: {}", e))?;

    if let Some(ref out_path) = args.output {
        fs::write(out_path, &output_str)?;
        println!("Report saved to {}", out_path.display().to_string().cyan());
    } else {
        println!("{}", output_str);
    }

    if args.check && report.overall_status.is_breach() {
        exit(1);
    }

    Ok(())
}

fn run_isolate_command(args: IsolateArgs) -> Result<()> {
    let target_files = discover_js_ts_files(&args.path);
    if target_files.is_empty() {
        println!("{} No JavaScript/TypeScript files found in {}", "ℹ".blue(), args.path.display());
        return Ok(());
    }

    let mut any_failing = false;
    for file in target_files {
        if let Ok(content) = fs::read_to_string(&file)
            && let Ok(rep) = flareperf::isolate::analyze_isolate_script(&file, &content) {
                println!("{}", flareperf::isolate::render_terminal_isolate(&rep));
                if !rep.is_passing(args.max_complexity) {
                    any_failing = true;
                }
            }
    }

    if args.check && any_failing {
        exit(1);
    }

    Ok(())
}

fn run_subrequests_command(args: SubrequestArgs) -> Result<()> {
    let target_files = discover_js_ts_files(&args.path);
    if target_files.is_empty() {
        println!("{} No JavaScript/TypeScript files found in {}", "ℹ".blue(), args.path.display());
        return Ok(());
    }

    let mut any_failing = false;
    for file in target_files {
        if let Ok(content) = fs::read_to_string(&file)
            && let Ok(rep) = flareperf::subrequests::analyze_subrequests(&file, &content) {
                println!("{}", flareperf::subrequests::render_terminal_subrequests(&rep));
                if !rep.is_passing() {
                    any_failing = true;
                }
            }
    }

    if args.check && any_failing {
        exit(1);
    }

    Ok(())
}

fn run_d1_command(args: D1Args) -> Result<()> {
    let target_files = discover_js_ts_files(&args.path);
    if target_files.is_empty() {
        println!("{} No JavaScript/TypeScript files found in {}", "ℹ".blue(), args.path.display());
        return Ok(());
    }

    let mut any_failing = false;
    for file in target_files {
        if let Ok(content) = fs::read_to_string(&file)
            && let Ok(rep) = flareperf::d1::analyze_d1_usage(&file, &content) {
                println!("{}", flareperf::d1::render_terminal_d1(&rep));
                if !rep.is_passing() {
                    any_failing = true;
                }
            }
    }

    if args.check && any_failing {
        exit(1);
    }

    Ok(())
}

fn run_check_command(args: CheckArgs) -> Result<()> {
    println!("{}", "⚡ Running Flareperf Complete Edge Performance Audit...\n".bold());

    // 1. Bundle Budget Check (if bundle/dist exists)
    let has_bundle = args.path.join("dist").exists() || args.path.join("_worker.js").exists() || args.path.extension().is_some_and(|e| e == "js" || e == "wasm");
    let mut bundle_breached = false;

    if has_bundle {
        println!("{}", "1. Auditing Edge Bundle Sizes & Quotas...".cyan().bold());
        let bundle_path = if args.path.join("dist").exists() { args.path.join("dist") } else { args.path.clone() };
        let cfg = flareperf::bundle::budget::BudgetConfig {
            preset: flareperf::bundle::budget::TierPreset::Free,
            limit_bytes: 1024 * 1024,
            compression: flareperf::bundle::compress::CompressionAlgo::Gzip,
            warn_threshold_pct: 80.0,
        };
        let options = flareperf::bundle::analyzer::AnalyzerOptions {
            target_path: bundle_path,
            budget_config: cfg,
            inspect_sourcemaps: true,
            inspect_wasm: true,
            top_packages: 5,
        };
        if let Ok(report) = flareperf::bundle::analyzer::analyze_target(&options) {
            if report.overall_status.is_breach() {
                bundle_breached = true;
            }
            if let Ok(out) = flareperf::bundle::report::render_report(&report, flareperf::bundle::report::OutputFormat::Terminal, true) {
                println!("{}", out);
            }
        }
    } else {
        println!("{} No dist/ or bundle artifact found. Skipping bundle budget check.", "ℹ".blue());
    }

    // 2. Isolate Cold-Start Analysis
    println!("\n{}", "2. Checking Isolate Initialization & Cold-Start Complexity...".cyan().bold());
    let js_files = discover_js_ts_files(&args.path);
    let mut isolate_issue = false;
    for file in &js_files {
        if let Ok(content) = fs::read_to_string(file)
            && let Ok(rep) = flareperf::isolate::analyze_isolate_script(file, &content)
                && !rep.is_passing(50) {
                    println!("{}", flareperf::isolate::render_terminal_isolate(&rep));
                    isolate_issue = true;
                }
    }
    if !isolate_issue {
        println!("{} Isolate cold-start patterns are clean across {} files!", "✓".green().bold(), js_files.len());
    }

    // 3. Subrequest Limits & Fanouts
    println!("\n{}", "3. Auditing 50-Subrequest Limits & Loop Fanouts...".cyan().bold());
    let mut subreq_issue = false;
    for file in &js_files {
        if let Ok(content) = fs::read_to_string(file)
            && let Ok(rep) = flareperf::subrequests::analyze_subrequests(file, &content)
                && !rep.is_passing() {
                    println!("{}", flareperf::subrequests::render_terminal_subrequests(&rep));
                    subreq_issue = true;
                }
    }
    if !subreq_issue {
        println!("{} No unbounded subrequest fan-out loops detected!", "✓".green().bold());
    }

    // 4. D1 Batching Checks
    println!("\n{}", "4. Checking D1 SQLite Query Batching...".cyan().bold());
    let mut d1_issue = false;
    for file in &js_files {
        if let Ok(content) = fs::read_to_string(file)
            && let Ok(rep) = flareperf::d1::analyze_d1_usage(file, &content)
                && !rep.is_passing() {
                    println!("{}", flareperf::d1::render_terminal_d1(&rep));
                    d1_issue = true;
                }
    }
    if !d1_issue {
        println!("{} No D1 N+1 query loop bottlenecks detected!", "✓".green().bold());
    }

    if args.strict && (bundle_breached || isolate_issue || subreq_issue || d1_issue) {
        println!("\n{} Edge performance check failed.", "✗".red().bold());
        exit(1);
    } else {
        println!("\n{} Edge performance check passed!", "✓".green().bold());
    }

    Ok(())
}

fn discover_js_ts_files(path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if path.is_file() {
        if let Some(ext) = path.extension()
            && (ext == "js" || ext == "ts" || ext == "mjs" || ext == "astro") {
                files.push(path.to_path_buf());
            }
        return files;
    }

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_file()
            && let Some(ext) = p.extension()
                && (ext == "js" || ext == "ts" || ext == "mjs" || ext == "astro") {
                    let s = p.to_string_lossy();
                    if !s.contains("node_modules") && !s.contains("target") && !s.contains(".git") {
                        files.push(p.to_path_buf());
                    }
                }
    }

    files
}
