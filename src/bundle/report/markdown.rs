use crate::bundle::analyzer::AuditReport;
use crate::bundle::budget::{format_bytes, BudgetStatus};

pub fn render_markdown(report: &AuditReport) -> String {
    let mut out = String::new();

    let (badge, title_status) = match report.overall_status {
        BudgetStatus::Pass => ("🟢", "PASSED"),
        BudgetStatus::Warning => ("🟡", "WARNING"),
        BudgetStatus::Breach => ("🔴", "BREACH"),
    };

    out.push_str(&format!("# {badge} Cloudflare Edge Bundle Budget: {title_status}\n\n"));
    out.push_str(&format!("**Target:** `{}`  \n", report.target_path));
    out.push_str(&format!("**Analyzed at:** `{}`  \n", report.analyzed_at));
    out.push_str(&format!("**Total Assets:** {} files\n\n", report.file_count));

    // Policy Card
    let eval = &report.main_budget_eval;
    out.push_str("## 🎯 Budget Policy Evaluation\n\n");
    out.push_str("| Metric | Value |\n");
    out.push_str("| :--- | :--- |\n");
    out.push_str(&format!("| **Policy / Tier** | {} |\n", eval.name));
    out.push_str(&format!("| **Evaluated Size** | {} ({}) |\n", format_bytes(eval.used_bytes), eval.compression));
    out.push_str(&format!("| **Budget Limit** | {} |\n", format_bytes(eval.limit_bytes)));
    out.push_str(&format!("| **Quota Utilization** | **{:.1}%** |\n", eval.usage_percentage));
    let headroom_str = if eval.headroom_bytes >= 0 {
        format!("{} remaining", format_bytes(eval.headroom_bytes as usize))
    } else {
        format!("**+{} OVER BUDGET**", format_bytes((-eval.headroom_bytes) as usize))
    };
    out.push_str(&format!("| **Headroom** | {} |\n", headroom_str));
    out.push_str(&format!("| **Status** | **{}** |\n\n", eval.status.as_str()));

    // Assets Summary Table
    out.push_str("## 📁 Asset Breakdown\n\n");
    out.push_str("| Asset Path | Type | Raw Size | Gzip (L9) | Brotli (Q11) | Zstd (L19) |\n");
    out.push_str("| :--- | :--- | :--- | :--- | :--- | :--- |\n");

    for file in &report.files {
        let name_display = if file.is_entrypoint {
            format!("⭐ `{}` *(entry)*", file.relative_path)
        } else {
            format!("`{}`", file.relative_path)
        };

        out.push_str(&format!(
            "| {} | {} | {} | {} ({:.0}%) | {} ({:.0}%) | {} ({:.0}%) |\n",
            name_display,
            file.file_type.display_name(),
            format_bytes(file.metrics.raw_bytes),
            format_bytes(file.metrics.gzip_bytes),
            file.metrics.gzip_ratio * 100.0,
            format_bytes(file.metrics.brotli_bytes),
            file.metrics.brotli_ratio * 100.0,
            format_bytes(file.metrics.zstd_bytes),
            file.metrics.zstd_ratio * 100.0,
        ));
    }

    out.push_str(&format!(
        "| **TOTAL** | **{} files** | **{}** | **{}** | **{}** | **{}** |\n\n",
        report.file_count,
        format_bytes(report.total_raw_bytes),
        format_bytes(report.total_gzip_bytes),
        format_bytes(report.total_brotli_bytes),
        format_bytes(report.total_zstd_bytes)
    ));

    // Sourcemaps
    for sm in &report.sourcemaps {
        out.push_str(&format!(
            "<details>\n<summary>🔍 <b>Sourcemap Breakdown:</b> <code>{}</code> (Mapped {:.1}%)</summary>\n\n",
            sm.sourcemap_path, sm.mapped_percentage
        ));

        out.push_str("| Package / Namespace | Generated Size | Share % |\n");
        out.push_str("| :--- | :--- | :--- |\n");

        for pkg in &sm.top_packages {
            out.push_str(&format!(
                "| `{}` | {} | **{:.1}%** |\n",
                pkg.package_name,
                format_bytes(pkg.byte_count),
                pkg.percentage
            ));
        }

        out.push_str("\n</details>\n\n");
    }

    // WASM modules
    for wasm in &report.wasm_modules {
        out.push_str(&format!(
            "<details>\n<summary>⚡ <b>WebAssembly Decomposition:</b> <code>{}</code> ({} total, Debug Symbols: {:.1}%)</summary>\n\n",
            wasm.file_path,
            format_bytes(wasm.total_size_bytes),
            wasm.debug_symbol_percentage
        ));

        out.push_str("| Section | Size | Share % | Details |\n");
        out.push_str("| :--- | :--- | :--- | :--- |\n");

        for sec in &wasm.sections {
            out.push_str(&format!(
                "| `{}` | {} | {:.1}% | {} |\n",
                sec.name,
                format_bytes(sec.size_bytes),
                sec.percentage,
                sec.details.as_deref().unwrap_or("-")
            ));
        }

        if !wasm.recommendations.is_empty() {
            out.push_str("\n**Optimization Recommendations:**\n");
            for rec in &wasm.recommendations {
                out.push_str(&format!("- {}\n", rec));
            }
        }

        out.push_str("\n</details>\n\n");
    }

    // Diagnostics / warnings
    if !report.diagnostics.is_empty() {
        out.push_str("## ⚠️ Diagnostics & Policy Warnings\n\n");
        for diag in &report.diagnostics {
            out.push_str(&format!("- {}\n", diag));
        }
        out.push('\n');
    }

    out
}
