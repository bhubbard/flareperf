use flareperf::d1::analyze_d1_usage;
use std::path::Path;

#[test]
fn test_d1_in_loop() {
    let code = "export default { async fetch(req, env) { for (const u of users) { await env.DB.prepare('SELECT * FROM users').all(); } } };";
    let rep = analyze_d1_usage(Path::new("index.ts"), code).unwrap();
    assert!(!rep.is_passing());
    assert!(rep.issues.iter().any(|i| i.rule_id == "D1-001"));
}
