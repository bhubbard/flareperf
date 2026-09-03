use flareperf::bundle::analyzer::{AnalyzerOptions, analyze_target};
use flareperf::bundle::budget::{BudgetConfig, TierPreset};
use flareperf::bundle::compress::CompressionAlgo;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_bundle_within_free_tier() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("_worker.js");
    fs::write(
        &file,
        "export default { async fetch() { return new Response('Hello'); } };",
    )
    .unwrap();

    let cfg = BudgetConfig {
        preset: TierPreset::Free,
        limit_bytes: 1024 * 1024,
        compression: CompressionAlgo::Gzip,
        warn_threshold_pct: 80.0,
    };

    let opts = AnalyzerOptions {
        target_path: dir.path().to_path_buf(),
        budget_config: cfg,
        inspect_sourcemaps: false,
        inspect_wasm: false,
        top_packages: 5,
    };

    let rep = analyze_target(&opts).unwrap();
    assert!(!rep.overall_status.is_breach());
}
