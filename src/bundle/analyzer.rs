use crate::bundle::budget::{
    BudgetConfig, BudgetEvaluation, BudgetStatus, CLOUDFLARE_PAGES_ASSET_BYTES,
};
use crate::bundle::compress::{CompressionAlgo, CompressionMetrics};
use crate::bundle::sourcemap::{SourcemapAnalysis, analyze_sourcemap, find_sourcemap_for_file};
use crate::bundle::wasm::{WasmAnalysis, analyze_wasm_file};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileType {
    WorkerScript,
    WasmModule,
    SourceMap,
    StaticAsset,
    Other,
}

impl FileType {
    pub fn from_path(path: &Path) -> Self {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.ends_with(".map") || name.ends_with(".js.map") || name.ends_with(".mjs.map") {
            return FileType::SourceMap;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        match ext.as_str() {
            "js" | "mjs" | "cjs" | "ts" | "jsx" | "tsx" => FileType::WorkerScript,
            "wasm" => FileType::WasmModule,
            "html" | "css" | "svg" | "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "woff"
            | "woff2" | "ttf" | "eot" | "ico" | "json" | "xml" | "txt" | "pdf" => {
                FileType::StaticAsset
            }
            _ => FileType::Other,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            FileType::WorkerScript => "Worker Script",
            FileType::WasmModule => "WASM Module",
            FileType::SourceMap => "Source Map",
            FileType::StaticAsset => "Static Asset",
            FileType::Other => "Other Asset",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzedFile {
    pub path: String,
    pub relative_path: String,
    pub file_type: FileType,
    pub metrics: CompressionMetrics,
    pub is_entrypoint: bool,
    pub pages_asset_budget: Option<BudgetEvaluation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub target_path: String,
    pub analyzed_at: String,
    pub total_raw_bytes: usize,
    pub total_gzip_bytes: usize,
    pub total_brotli_bytes: usize,
    pub total_zstd_bytes: usize,
    pub file_count: usize,
    pub files: Vec<AnalyzedFile>,
    pub main_budget_eval: BudgetEvaluation,
    pub pages_asset_breaches: Vec<BudgetEvaluation>,
    pub sourcemaps: Vec<SourcemapAnalysis>,
    pub wasm_modules: Vec<WasmAnalysis>,
    pub overall_status: BudgetStatus,
    pub diagnostics: Vec<String>,
}

pub struct AnalyzerOptions {
    pub target_path: PathBuf,
    pub budget_config: BudgetConfig,
    pub inspect_sourcemaps: bool,
    pub inspect_wasm: bool,
    pub top_packages: usize,
}

pub fn analyze_target(options: &AnalyzerOptions) -> Result<AuditReport, String> {
    let target = &options.target_path;
    if !target.exists() {
        return Err(format!("Target path '{}' does not exist", target.display()));
    }

    let mut file_paths: Vec<PathBuf> = Vec::new();
    if target.is_file() {
        file_paths.push(target.clone());
    } else {
        for entry in WalkDir::new(target)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Skip hidden directories and .git, node_modules (unless targeted directly)
            let path_str = path.to_string_lossy();
            if path_str.contains("/.git/")
                || path_str.contains("/node_modules/")
                || path_str.contains("/.wrangler/tmp/")
                || name.starts_with('.')
            {
                continue;
            }

            file_paths.push(path.to_path_buf());
        }
    }

    if file_paths.is_empty() {
        return Err(format!("No files found in '{}'", target.display()));
    }

    file_paths.sort();

    let mut analyzed_files: Vec<AnalyzedFile> = Vec::new();
    let mut total_raw_bytes = 0usize;
    let mut total_gzip_bytes = 0usize;
    let mut total_brotli_bytes = 0usize;
    let mut total_zstd_bytes = 0usize;
    let mut pages_asset_breaches = Vec::new();

    let mut sourcemaps_to_analyze: Vec<PathBuf> = Vec::new();
    let mut wasm_to_analyze: Vec<PathBuf> = Vec::new();

    // Identify entrypoint heuristic
    // If single file -> entrypoint.
    // If directory -> look for index.js, worker.js, entry.js, index.mjs, or largest script
    let mut candidate_entrypoint_idx: Option<usize> = None;
    let mut max_script_size = 0usize;

    for (idx, path) in file_paths.iter().enumerate() {
        let file_type = FileType::from_path(path);
        let rel_path = if target.is_dir() {
            path.strip_prefix(target)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string()
        } else {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
                .to_string()
        };

        let data =
            fs::read(path).map_err(|e| format!("Failed to read file '{}': {e}", path.display()))?;
        let metrics = CompressionMetrics::calculate(&data);

        total_raw_bytes += metrics.raw_bytes;
        total_gzip_bytes += metrics.gzip_bytes;
        total_brotli_bytes += metrics.brotli_bytes;
        total_zstd_bytes += metrics.zstd_bytes;

        // Pages asset 25MB single file limit check
        let pages_eval = if metrics.raw_bytes > CLOUDFLARE_PAGES_ASSET_BYTES {
            let eval = BudgetEvaluation::evaluate(
                &format!("Pages Asset: {rel_path}"),
                &rel_path,
                CLOUDFLARE_PAGES_ASSET_BYTES,
                metrics.raw_bytes,
                CompressionAlgo::Raw,
                80.0,
            );
            pages_asset_breaches.push(eval.clone());
            Some(eval)
        } else {
            None
        };

        if file_type == FileType::SourceMap {
            sourcemaps_to_analyze.push(path.clone());
        } else if file_type == FileType::WasmModule {
            wasm_to_analyze.push(path.clone());
        }

        if file_type == FileType::WorkerScript || file_type == FileType::WasmModule {
            let name_lower = rel_path.to_ascii_lowercase();
            if name_lower.ends_with("worker.js")
                || name_lower.ends_with("index.js")
                || name_lower.ends_with("index.mjs")
                || name_lower.ends_with("entry.js")
                || name_lower.ends_with("_worker.js")
            {
                candidate_entrypoint_idx = Some(idx);
            } else if candidate_entrypoint_idx.is_none() && metrics.raw_bytes > max_script_size {
                max_script_size = metrics.raw_bytes;
                candidate_entrypoint_idx = Some(idx);
            }
        }

        analyzed_files.push(AnalyzedFile {
            path: path.to_string_lossy().to_string(),
            relative_path: rel_path,
            file_type,
            metrics,
            is_entrypoint: false,
            pages_asset_budget: pages_eval,
        });
    }

    if let Some(idx) = candidate_entrypoint_idx {
        if idx < analyzed_files.len() {
            analyzed_files[idx].is_entrypoint = true;
        }
    } else if !analyzed_files.is_empty() {
        analyzed_files[0].is_entrypoint = true;
    }

    // Evaluate main budget
    // If auditing a single file, evaluate that file.
    // If auditing a directory, evaluate the total bundle size (or primary worker script size)
    let evaluated_bytes = if target.is_file() {
        analyzed_files[0]
            .metrics
            .size_for(options.budget_config.compression)
    } else {
        // If directory has worker scripts, sum WorkerScript + WasmModule compressed sizes for Worker quota
        let bundle_compressed: usize = analyzed_files
            .iter()
            .filter(|f| {
                f.file_type == FileType::WorkerScript || f.file_type == FileType::WasmModule
            })
            .map(|f| f.metrics.size_for(options.budget_config.compression))
            .sum();

        if bundle_compressed > 0 {
            bundle_compressed
        } else {
            // Otherwise use total compression of directory
            match options.budget_config.compression {
                CompressionAlgo::Gzip => total_gzip_bytes,
                CompressionAlgo::Brotli => total_brotli_bytes,
                CompressionAlgo::Zstd => total_zstd_bytes,
                CompressionAlgo::Raw => total_raw_bytes,
                CompressionAlgo::All => total_gzip_bytes,
            }
        }
    };

    let main_budget_eval = BudgetEvaluation::evaluate(
        options.budget_config.preset.display_name(),
        &target.to_string_lossy(),
        options.budget_config.limit_bytes,
        evaluated_bytes,
        options.budget_config.compression,
        options.budget_config.warn_threshold_pct,
    );

    // Sourcemap analysis
    let mut sourcemap_results: Vec<SourcemapAnalysis> = Vec::new();
    if options.inspect_sourcemaps {
        // 1. Explicit .map files
        for sm_path in &sourcemaps_to_analyze {
            if let Ok(analysis) = analyze_sourcemap(sm_path, None) {
                sourcemap_results.push(analysis);
            }
        }

        // 2. Discover .map for scripts if not already parsed
        for file in &analyzed_files {
            if file.file_type == FileType::WorkerScript {
                let p = Path::new(&file.path);
                if let Some(map_path) = find_sourcemap_for_file(p)
                    && !sourcemaps_to_analyze.contains(&map_path)
                    && let Ok(analysis) = analyze_sourcemap(&map_path, None)
                {
                    sourcemap_results.push(analysis);
                }
            }
        }
    }

    // WASM analysis
    let mut wasm_results: Vec<WasmAnalysis> = Vec::new();
    if options.inspect_wasm {
        for wasm_path in &wasm_to_analyze {
            if let Ok(analysis) = analyze_wasm_file(wasm_path) {
                wasm_results.push(analysis);
            }
        }
    }

    // Diagnostics / rules checks
    let mut diagnostics = Vec::new();

    if main_budget_eval.status == BudgetStatus::Breach {
        diagnostics.push(format!(
            "CF-BUDGET-001: Bundle compressed size ({}) exceeds {} limit of {}",
            crate::bundle::budget::format_bytes(main_budget_eval.used_bytes),
            options.budget_config.preset.display_name(),
            crate::bundle::budget::format_bytes(main_budget_eval.limit_bytes),
        ));
    } else if main_budget_eval.status == BudgetStatus::Warning {
        diagnostics.push(format!(
            "CF-BUDGET-004: Bundle compressed size is approaching {} limit ({:.1}% used)",
            options.budget_config.preset.display_name(),
            main_budget_eval.usage_percentage
        ));
    }

    for breach in &pages_asset_breaches {
        diagnostics.push(format!(
            "CF-BUDGET-002: Static asset '{}' size ({}) exceeds Cloudflare Pages 25 MB single asset limit",
            breach.target_path,
            crate::bundle::budget::format_bytes(breach.used_bytes)
        ));
    }

    for wasm in &wasm_results {
        if wasm.debug_symbol_bytes > 1024 * 10 || wasm.debug_symbol_percentage > 10.0 {
            diagnostics.push(format!(
                "CF-BUDGET-003: WASM module '{}' contains {} of unstripped debug symbols ({:.1}% of binary)",
                wasm.file_path,
                crate::bundle::budget::format_bytes(wasm.debug_symbol_bytes),
                wasm.debug_symbol_percentage
            ));
        }
    }

    for sm in &sourcemap_results {
        for pkg in &sm.packages {
            if pkg.percentage > 30.0 && pkg.package_name != "[app source]" {
                diagnostics.push(format!(
                    "CF-BUDGET-005: Package '{}' accounts for {:.1}% ({} B) of bundle in '{}'",
                    pkg.package_name, pkg.percentage, pkg.byte_count, sm.sourcemap_path
                ));
            }
        }
    }

    let mut overall_status = main_budget_eval.status;
    if !pages_asset_breaches.is_empty() {
        overall_status = BudgetStatus::Breach;
    }

    Ok(AuditReport {
        target_path: target.to_string_lossy().to_string(),
        analyzed_at: "2026-08-22T16:00:00Z".to_string(),
        total_raw_bytes,
        total_gzip_bytes,
        total_brotli_bytes,
        total_zstd_bytes,
        file_count: analyzed_files.len(),
        files: analyzed_files,
        main_budget_eval,
        pages_asset_breaches,
        sourcemaps: sourcemap_results,
        wasm_modules: wasm_results,
        overall_status,
        diagnostics,
    })
}
