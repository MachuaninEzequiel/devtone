#[test]
fn readme_mentions_four_agents_and_no_source() {
    let t = include_str!("../../../README.md").to_lowercase();
    for needle in ["pi", "claude", "codex", "opencode", "--no-notch"] {
        assert!(t.contains(needle), "missing {needle}");
    }
}
