pub mod json;
pub mod markdown;
pub mod sarif;
pub mod terminal;

use crate::bundle::analyzer::AuditReport;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Terminal,
    Json,
    Markdown,
    Sarif,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "terminal" | "text" | "cli" => Ok(OutputFormat::Terminal),
            "json" => Ok(OutputFormat::Json),
            "markdown" | "md" => Ok(OutputFormat::Markdown),
            "sarif" => Ok(OutputFormat::Sarif),
            _ => Err(format!(
                "Unknown format: '{s}'. Supported: terminal, json, markdown, sarif"
            )),
        }
    }
}

pub fn render_report(
    report: &AuditReport,
    format: OutputFormat,
    use_color: bool,
) -> Result<String, String> {
    match format {
        OutputFormat::Terminal => Ok(terminal::render_terminal(report, use_color)),
        OutputFormat::Json => json::render_json(report),
        OutputFormat::Markdown => Ok(markdown::render_markdown(report)),
        OutputFormat::Sarif => sarif::render_sarif(report),
    }
}
