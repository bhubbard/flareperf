use crate::bundle::compress::CompressionAlgo;
use serde::{Deserialize, Serialize};

pub const CLOUDFLARE_FREE_TIER_BYTES: usize = 1024 * 1024; // 1 MB = 1,048,576 bytes
pub const CLOUDFLARE_PAID_TIER_BYTES: usize = 10 * 1024 * 1024; // 10 MB = 10,485,760 bytes
pub const CLOUDFLARE_WORKERS_AI_BYTES: usize = 5 * 1024 * 1024; // 5 MB = 5,242,880 bytes
pub const CLOUDFLARE_PAGES_ASSET_BYTES: usize = 25 * 1024 * 1024; // 25 MB = 26,214,400 bytes

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TierPreset {
    Free,
    Paid,
    WorkersAi,
    PagesAsset,
    Custom,
    None,
}

impl TierPreset {
    pub fn default_limit_bytes(&self) -> Option<usize> {
        match self {
            TierPreset::Free => Some(CLOUDFLARE_FREE_TIER_BYTES),
            TierPreset::Paid => Some(CLOUDFLARE_PAID_TIER_BYTES),
            TierPreset::WorkersAi => Some(CLOUDFLARE_WORKERS_AI_BYTES),
            TierPreset::PagesAsset => Some(CLOUDFLARE_PAGES_ASSET_BYTES),
            TierPreset::Custom | TierPreset::None => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            TierPreset::Free => "Cloudflare Free Tier (1 MB compressed)",
            TierPreset::Paid => "Cloudflare Paid / Standard Tier (10 MB compressed)",
            TierPreset::WorkersAi => "Cloudflare Workers AI / Vectorize (5 MB compressed)",
            TierPreset::PagesAsset => "Cloudflare Pages Static Asset (25 MB limit)",
            TierPreset::Custom => "Custom Budget",
            TierPreset::None => "No Tier Limit",
        }
    }
}

impl std::str::FromStr for TierPreset {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().replace('_', "-").as_str() {
            "free" => Ok(TierPreset::Free),
            "paid" | "standard" => Ok(TierPreset::Paid),
            "workers-ai" | "ai" | "vectorize" => Ok(TierPreset::WorkersAi),
            "pages-asset" | "pages" => Ok(TierPreset::PagesAsset),
            "custom" => Ok(TierPreset::Custom),
            "none" | "off" => Ok(TierPreset::None),
            _ => Err(format!(
                "Unknown tier preset: '{s}'. Valid values: free, paid, workers-ai, pages-asset, custom, none"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum BudgetStatus {
    Pass,
    Warning,
    Breach,
}

impl BudgetStatus {
    pub fn is_breach(&self) -> bool {
        matches!(self, BudgetStatus::Breach)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            BudgetStatus::Pass => "PASS",
            BudgetStatus::Warning => "WARN",
            BudgetStatus::Breach => "BREACH",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    pub preset: TierPreset,
    pub limit_bytes: usize,
    pub compression: CompressionAlgo,
    pub warn_threshold_pct: f64,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            preset: TierPreset::Free,
            limit_bytes: CLOUDFLARE_FREE_TIER_BYTES,
            compression: CompressionAlgo::Gzip,
            warn_threshold_pct: 80.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetEvaluation {
    pub name: String,
    pub target_path: String,
    pub limit_bytes: usize,
    pub used_bytes: usize,
    pub compression: CompressionAlgo,
    pub usage_percentage: f64,
    pub headroom_bytes: i64,
    pub status: BudgetStatus,
    pub message: String,
}

impl BudgetEvaluation {
    pub fn evaluate(
        name: &str,
        target_path: &str,
        limit_bytes: usize,
        used_bytes: usize,
        compression: CompressionAlgo,
        warn_threshold_pct: f64,
    ) -> Self {
        let usage_percentage = if limit_bytes > 0 {
            (used_bytes as f64 / limit_bytes as f64) * 100.0
        } else {
            0.0
        };

        let headroom_bytes = limit_bytes as i64 - used_bytes as i64;

        let status = if used_bytes > limit_bytes {
            BudgetStatus::Breach
        } else if usage_percentage >= warn_threshold_pct {
            BudgetStatus::Warning
        } else {
            BudgetStatus::Pass
        };

        let message = match status {
            BudgetStatus::Pass => format!(
                "Within budget: {:.1}% used ({} remaining)",
                usage_percentage,
                format_bytes(headroom_bytes.max(0) as usize)
            ),
            BudgetStatus::Warning => format!(
                "Warning: {:.1}% of budget used! ({} remaining, threshold is {:.0}%)",
                usage_percentage,
                format_bytes(headroom_bytes.max(0) as usize),
                warn_threshold_pct
            ),
            BudgetStatus::Breach => format!(
                "BUDGET BREACH: {:.1}% used! Exceeds limit by {}",
                usage_percentage,
                format_bytes((-headroom_bytes).max(0) as usize)
            ),
        };

        Self {
            name: name.to_string(),
            target_path: target_path.to_string(),
            limit_bytes,
            used_bytes,
            compression,
            usage_percentage,
            headroom_bytes,
            status,
            message,
        }
    }
}

pub fn parse_size_str(s: &str) -> Result<usize, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Size string cannot be empty".to_string());
    }

    let s_lower = s.to_ascii_lowercase();

    // Check for units
    let (num_part, multiplier) = if s_lower.ends_with("gib") || s_lower.ends_with("gb") {
        let num_str = s[..s.len() - if s_lower.ends_with("gib") { 3 } else { 2 }].trim();
        (num_str, 1024.0 * 1024.0 * 1024.0)
    } else if s_lower.ends_with("mib") || s_lower.ends_with("mb") {
        let num_str = s[..s.len() - if s_lower.ends_with("mib") { 3 } else { 2 }].trim();
        (num_str, 1024.0 * 1024.0)
    } else if s_lower.ends_with("kib") || s_lower.ends_with("kb") {
        let num_str = s[..s.len() - if s_lower.ends_with("kib") { 3 } else { 2 }].trim();
        (num_str, 1024.0)
    } else if s_lower.ends_with('b') {
        let num_str = s[..s.len() - 1].trim();
        (num_str, 1.0)
    } else {
        (s, 1.0)
    };

    num_part
        .parse::<f64>()
        .map(|val| (val * multiplier).round() as usize)
        .map_err(|e| format!("Failed to parse size '{s}': {e}"))
}

pub fn format_bytes(bytes: usize) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

    let b = bytes as f64;
    if b >= GIB {
        format!("{:.2} GiB ({} B)", b / GIB, bytes)
    } else if b >= MIB {
        format!("{:.2} MiB ({} B)", b / MIB, bytes)
    } else if b >= KIB {
        format!("{:.2} KiB ({} B)", b / KIB, bytes)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_bytes_compact(bytes: usize) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

    let b = bytes as f64;
    if b >= GIB {
        format!("{:.2} GiB", b / GIB)
    } else if b >= MIB {
        format!("{:.2} MiB", b / MIB)
    } else if b >= KIB {
        format!("{:.1} KiB", b / KIB)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sizes() {
        assert_eq!(parse_size_str("1024").unwrap(), 1024);
        assert_eq!(parse_size_str("1024B").unwrap(), 1024);
        assert_eq!(parse_size_str("1KB").unwrap(), 1024);
        assert_eq!(parse_size_str("1KiB").unwrap(), 1024);
        assert_eq!(parse_size_str("1MB").unwrap(), 1024 * 1024);
        assert_eq!(parse_size_str("10MB").unwrap(), 10 * 1024 * 1024);
        assert_eq!(parse_size_str("1.5MB").unwrap(), (1.5 * 1024.0 * 1024.0) as usize);
        assert_eq!(parse_size_str("25MB").unwrap(), 25 * 1024 * 1024);
    }

    #[test]
    fn test_budget_evaluation() {
        let pass = BudgetEvaluation::evaluate(
            "Worker Script",
            "dist/worker.js",
            1024 * 1024,
            500 * 1024,
            CompressionAlgo::Gzip,
            80.0,
        );
        assert_eq!(pass.status, BudgetStatus::Pass);
        assert!(!pass.status.is_breach());

        let warn = BudgetEvaluation::evaluate(
            "Worker Script",
            "dist/worker.js",
            1024 * 1024,
            850 * 1024,
            CompressionAlgo::Gzip,
            80.0,
        );
        assert_eq!(warn.status, BudgetStatus::Warning);
        assert!(!warn.status.is_breach());

        let breach = BudgetEvaluation::evaluate(
            "Worker Script",
            "dist/worker.js",
            1024 * 1024,
            1200 * 1024,
            CompressionAlgo::Gzip,
            80.0,
        );
        assert_eq!(breach.status, BudgetStatus::Breach);
        assert!(breach.status.is_breach());
    }
}
