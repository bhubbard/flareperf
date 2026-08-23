use crate::bundle::analyzer::AuditReport;
use crate::bundle::budget::{format_bytes, BudgetStatus};
use colored::Colorize;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table};

pub fn render_terminal(report: &AuditReport, use_color: bool) -> String {
    let mut out = String::new();

    // Title banner
    let title = "📦 Cloudflare Edge Bundle Budget Auditor";
    if use_color {
        out.push_str(&format!("\n{}\n", title.bold().cyan()));
    } else {
        out.push_str(&format!("\n{title}\n"));
    }

    out.push_str(&format!("Target: {}\n", report.target_path));

    // Budget Compliance Card
    let eval = &report.main_budget_eval;
    let (status_str, status_color) = match eval.status {
        BudgetStatus::Pass => ("PASS", Color::Green),
        BudgetStatus::Warning => ("WARN", Color::Yellow),
        BudgetStatus::Breach => ("BREACH", Color::Red),
    };

    let mut budget_table = Table::new();
    budget_table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic);

    budget_table.set_header(vec![
        Cell::new("Tier / Budget Policy").add_attribute(Attribute::Bold),
        Cell::new("Evaluated Size").add_attribute(Attribute::Bold),
        Cell::new("Limit").add_attribute(Attribute::Bold),
        Cell::new("Usage").add_attribute(Attribute::Bold),
        Cell::new("Headroom").add_attribute(Attribute::Bold),
        Cell::new("Status").add_attribute(Attribute::Bold),
    ]);

    let headroom_str = if eval.headroom_bytes >= 0 {
        format!("{} remaining", format_bytes(eval.headroom_bytes as usize))
    } else {
        format!("+{} over limit", format_bytes((-eval.headroom_bytes) as usize))
    };

    let usage_bar = render_ascii_bar(eval.usage_percentage, 15);

    let status_cell = if use_color {
        Cell::new(status_str)
            .fg(status_color)
            .add_attribute(Attribute::Bold)
    } else {
        Cell::new(status_str)
    };

    budget_table.add_row(vec![
        Cell::new(&eval.name),
        Cell::new(format!("{} ({})", format_bytes(eval.used_bytes), eval.compression)),
        Cell::new(format_bytes(eval.limit_bytes)),
        Cell::new(format!("{:.1}% {}", eval.usage_percentage, usage_bar)),
        Cell::new(headroom_str),
        status_cell,
    ]);

    out.push_str(&budget_table.to_string());
    out.push('\n');

    // Files breakdown table
    out.push_str("\n📁 Analyzed Assets Breakdown:\n");
    let mut files_table = Table::new();
    files_table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic);

    files_table.set_header(vec![
        Cell::new("Asset Path").add_attribute(Attribute::Bold),
        Cell::new("Type").add_attribute(Attribute::Bold),
        Cell::new("Raw Size").add_attribute(Attribute::Bold),
        Cell::new("Gzip (L9)").add_attribute(Attribute::Bold),
        Cell::new("Brotli (Q11)").add_attribute(Attribute::Bold),
        Cell::new("Zstd (L19)").add_attribute(Attribute::Bold),
    ]);

    for file in &report.files {
        let name_display = if file.is_entrypoint {
            format!("* {}", file.relative_path)
        } else {
            file.relative_path.clone()
        };

        files_table.add_row(vec![
            Cell::new(name_display),
            Cell::new(file.file_type.display_name()),
            Cell::new(format_bytes(file.metrics.raw_bytes)),
            Cell::new(format!(
                "{} ({:.0}%)",
                format_bytes(file.metrics.gzip_bytes),
                file.metrics.gzip_ratio * 100.0
            )),
            Cell::new(format!(
                "{} ({:.0}%)",
                format_bytes(file.metrics.brotli_bytes),
                file.metrics.brotli_ratio * 100.0
            )),
            Cell::new(format!(
                "{} ({:.0}%)",
                format_bytes(file.metrics.zstd_bytes),
                file.metrics.zstd_ratio * 100.0
            )),
        ]);
    }

    // Totals row
    files_table.add_row(vec![
        Cell::new("TOTAL BUNDLE").add_attribute(Attribute::Bold),
        Cell::new(format!("{} files", report.file_count)),
        Cell::new(format_bytes(report.total_raw_bytes)).add_attribute(Attribute::Bold),
        Cell::new(format_bytes(report.total_gzip_bytes)).add_attribute(Attribute::Bold),
        Cell::new(format_bytes(report.total_brotli_bytes)).add_attribute(Attribute::Bold),
        Cell::new(format_bytes(report.total_zstd_bytes)).add_attribute(Attribute::Bold),
    ]);

    out.push_str(&files_table.to_string());
    out.push('\n');

    // Sourcemap packages breakdown
    for sm in &report.sourcemaps {
        out.push_str(&format!(
            "\n🔍 Sourcemap Inspector: {}\n",
            sm.sourcemap_path
        ));
        out.push_str(&format!(
            "Mapped: {} / {} ({:.1}%)\n",
            format_bytes(sm.total_mapped_bytes),
            format_bytes(sm.total_generated_bytes),
            sm.mapped_percentage
        ));

        let mut sm_table = Table::new();
        sm_table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::Dynamic);

        sm_table.set_header(vec![
            Cell::new("Package / Namespace").add_attribute(Attribute::Bold),
            Cell::new("Generated Size").add_attribute(Attribute::Bold),
            Cell::new("Share %").add_attribute(Attribute::Bold),
            Cell::new("Distribution").add_attribute(Attribute::Bold),
        ]);

        for pkg in &sm.top_packages {
            let bar = render_ascii_bar(pkg.percentage, 20);
            sm_table.add_row(vec![
                Cell::new(&pkg.package_name),
                Cell::new(format_bytes(pkg.byte_count)),
                Cell::new(format!("{:.1}%", pkg.percentage)),
                Cell::new(bar),
            ]);
        }

        out.push_str(&sm_table.to_string());
        out.push('\n');
    }

    // WebAssembly breakdown
    for wasm in &report.wasm_modules {
        out.push_str(&format!(
            "\n⚡ WebAssembly Decomposition: {}\n",
            wasm.file_path
        ));
        out.push_str(&format!(
            "Total: {} | Code: {} | Data: {} | Debug Symbols: {} ({:.1}%)\n",
            format_bytes(wasm.total_size_bytes),
            format_bytes(wasm.code_section_bytes),
            format_bytes(wasm.data_section_bytes),
            format_bytes(wasm.debug_symbol_bytes),
            wasm.debug_symbol_percentage
        ));

        let mut wasm_table = Table::new();
        wasm_table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::Dynamic);

        wasm_table.set_header(vec![
            Cell::new("Section").add_attribute(Attribute::Bold),
            Cell::new("Size").add_attribute(Attribute::Bold),
            Cell::new("Share %").add_attribute(Attribute::Bold),
            Cell::new("Details").add_attribute(Attribute::Bold),
        ]);

        for sec in &wasm.sections {
            wasm_table.add_row(vec![
                Cell::new(&sec.name),
                Cell::new(format_bytes(sec.size_bytes)),
                Cell::new(format!("{:.1}%", sec.percentage)),
                Cell::new(sec.details.as_deref().unwrap_or("-")),
            ]);
        }

        out.push_str(&wasm_table.to_string());
        out.push('\n');

        if !wasm.recommendations.is_empty() {
            out.push_str("💡 Optimization Recommendations:\n");
            for rec in &wasm.recommendations {
                if use_color {
                    out.push_str(&format!("  • {}\n", rec.cyan()));
                } else {
                    out.push_str(&format!("  • {rec}\n"));
                }
            }
        }
    }

    // Diagnostics / warnings
    if !report.diagnostics.is_empty() {
        out.push_str("\n⚠️  Diagnostics & Policy Warnings:\n");
        for diag in &report.diagnostics {
            if use_color {
                if diag.contains("CF-BUDGET-001") || diag.contains("CF-BUDGET-002") {
                    out.push_str(&format!("  {} {}\n", "✖".red().bold(), diag.red()));
                } else {
                    out.push_str(&format!("  {} {}\n", "▲".yellow().bold(), diag.yellow()));
                }
            } else {
                out.push_str(&format!("  - {diag}\n"));
            }
        }
        out.push('\n');
    }

    out
}

fn render_ascii_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "■".repeat(filled), " ".repeat(empty))
}
