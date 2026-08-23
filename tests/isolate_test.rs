use flareperf::isolate::analyze_isolate_script;
use std::path::Path;

#[test]
fn test_clean_isolate_worker() {
    let code = "export default { async fetch(req, env, ctx) { return new Response('ok'); } };";
    let rep = analyze_isolate_script(Path::new("index.ts"), code).unwrap();
    assert!(rep.is_passing(50));
    assert_eq!(rep.issues.len(), 0);
}

#[test]
fn test_heavy_top_level_loop() {
    let code = "for (let i = 0; i < 1000; i++) { console.log(i); }
export default { async fetch() {} };";
    let rep = analyze_isolate_script(Path::new("index.ts"), code).unwrap();
    assert!(!rep.issues.is_empty());
    assert!(rep.issues.iter().any(|i| i.rule_id == "ISO-001"));
}
