use crate::bundle::analyzer::AuditReport;

pub fn render_json(report: &AuditReport) -> Result<String, String> {
    serde_json::to_string_pretty(report).map_err(|e| format!("Failed to serialize JSON report: {e}"))
}
