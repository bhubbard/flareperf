use crate::bundle::analyzer::AuditReport;
use crate::bundle::budget::BudgetStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifReport {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub version: String,
    pub runs: Vec<SarifRun>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifRun {
    pub tool: SarifTool,
    pub results: Vec<SarifResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifTool {
    pub driver: SarifDriver,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifDriver {
    pub name: String,
    pub version: String,
    pub information_uri: String,
    pub rules: Vec<SarifRule>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifRule {
    pub id: String,
    pub name: String,
    pub short_description: SarifMessage,
    pub default_configuration: SarifConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifConfig {
    pub level: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifResult {
    pub rule_id: String,
    pub level: String,
    pub message: SarifMessage,
    pub locations: Vec<SarifLocation>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifMessage {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifLocation {
    pub physical_location: SarifPhysicalLocation,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifPhysicalLocation {
    pub artifact_location: SarifArtifactLocation,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifArtifactLocation {
    pub uri: String,
}

pub fn render_sarif(report: &AuditReport) -> Result<String, String> {
    let rules = vec![
        SarifRule {
            id: "CF-BUDGET-001".to_string(),
            name: "TierQuotaBreach".to_string(),
            short_description: SarifMessage {
                text: "Cloudflare Worker compressed bundle exceeds tier quota".to_string(),
            },
            default_configuration: SarifConfig {
                level: "error".to_string(),
            },
        },
        SarifRule {
            id: "CF-BUDGET-002".to_string(),
            name: "PagesAssetSizeLimit".to_string(),
            short_description: SarifMessage {
                text: "Static asset exceeds Cloudflare Pages 25 MB single asset limit".to_string(),
            },
            default_configuration: SarifConfig {
                level: "error".to_string(),
            },
        },
        SarifRule {
            id: "CF-BUDGET-003".to_string(),
            name: "WasmDebugSymbols".to_string(),
            short_description: SarifMessage {
                text: "WebAssembly binary contains unstripped debug symbols".to_string(),
            },
            default_configuration: SarifConfig {
                level: "warning".to_string(),
            },
        },
        SarifRule {
            id: "CF-BUDGET-004".to_string(),
            name: "QuotaWarningThreshold".to_string(),
            short_description: SarifMessage {
                text: "Bundle size is approaching quota threshold".to_string(),
            },
            default_configuration: SarifConfig {
                level: "warning".to_string(),
            },
        },
        SarifRule {
            id: "CF-BUDGET-005".to_string(),
            name: "DominantPackageWeight".to_string(),
            short_description: SarifMessage {
                text: "Single dependency accounts for >30% of total bundle size".to_string(),
            },
            default_configuration: SarifConfig {
                level: "note".to_string(),
            },
        },
    ];

    let mut results = Vec::new();

    if report.main_budget_eval.status == BudgetStatus::Breach {
        results.push(SarifResult {
            rule_id: "CF-BUDGET-001".to_string(),
            level: "error".to_string(),
            message: SarifMessage {
                text: report.main_budget_eval.message.clone(),
            },
            locations: vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifactLocation {
                        uri: report.target_path.clone(),
                    },
                },
            }],
        });
    } else if report.main_budget_eval.status == BudgetStatus::Warning {
        results.push(SarifResult {
            rule_id: "CF-BUDGET-004".to_string(),
            level: "warning".to_string(),
            message: SarifMessage {
                text: report.main_budget_eval.message.clone(),
            },
            locations: vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifactLocation {
                        uri: report.target_path.clone(),
                    },
                },
            }],
        });
    }

    for breach in &report.pages_asset_breaches {
        results.push(SarifResult {
            rule_id: "CF-BUDGET-002".to_string(),
            level: "error".to_string(),
            message: SarifMessage {
                text: breach.message.clone(),
            },
            locations: vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifactLocation {
                        uri: breach.target_path.clone(),
                    },
                },
            }],
        });
    }

    for wasm in &report.wasm_modules {
        if wasm.debug_symbol_bytes > 1024 * 10 || wasm.debug_symbol_percentage > 10.0 {
            results.push(SarifResult {
                rule_id: "CF-BUDGET-003".to_string(),
                level: "warning".to_string(),
                message: SarifMessage {
                    text: format!(
                        "WASM module contains {} of debug symbols ({:.1}%). Optimization: wasm-opt -O3 --strip-debug",
                        crate::bundle::budget::format_bytes(wasm.debug_symbol_bytes),
                        wasm.debug_symbol_percentage
                    ),
                },
                locations: vec![SarifLocation {
                    physical_location: SarifPhysicalLocation {
                        artifact_location: SarifArtifactLocation {
                            uri: wasm.file_path.clone(),
                        },
                    },
                }],
            });
        }
    }

    for sm in &report.sourcemaps {
        for pkg in &sm.packages {
            if pkg.percentage > 30.0 && pkg.package_name != "[app source]" {
                results.push(SarifResult {
                    rule_id: "CF-BUDGET-005".to_string(),
                    level: "note".to_string(),
                    message: SarifMessage {
                        text: format!(
                            "Dependency '{}' accounts for {:.1}% ({} B) of bundle",
                            pkg.package_name, pkg.percentage, pkg.byte_count
                        ),
                    },
                    locations: vec![SarifLocation {
                        physical_location: SarifPhysicalLocation {
                            artifact_location: SarifArtifactLocation {
                                uri: sm.sourcemap_path.clone(),
                            },
                        },
                    }],
                });
            }
        }
    }

    let sarif = SarifReport {
        schema: "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json".to_string(),
        version: "2.1.0".to_string(),
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "edge-bundle-budget".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    information_uri: "https://github.com/cloudflare/edge-bundle-budget".to_string(),
                    rules,
                },
            },
            results,
        }],
    };

    serde_json::to_string_pretty(&sarif)
        .map_err(|e| format!("Failed to serialize SARIF report: {e}"))
}
