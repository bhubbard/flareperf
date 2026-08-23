use flareperf::subrequests::analyze_subrequests;
use std::path::Path;

#[test]
fn test_subrequest_in_loop() {
    let code = "export default { async fetch() { for (const id of [1, 2]) { await fetch('https://api/' + id); } } };";
    let rep = analyze_subrequests(Path::new("index.ts"), code).unwrap();
    assert!(!rep.is_passing());
    assert!(rep.issues.iter().any(|i| i.rule_id == "SUB-001"));
}
