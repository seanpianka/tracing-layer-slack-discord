const READMES: [(&str, &str); 3] = [
    ("README.md", include_str!("../../README.md")),
    (
        "layers/discord/README.md",
        include_str!("../../layers/discord/README.md"),
    ),
    ("layers/slack/README.md", include_str!("../../layers/slack/README.md")),
];

#[test]
fn rust_examples_do_not_hide_required_lines() {
    for (path, readme) in READMES {
        let mut in_rust_block = false;
        for (index, line) in readme.lines().enumerate() {
            if line.starts_with("```rust") {
                in_rust_block = true;
            } else if line == "```" {
                in_rust_block = false;
            } else if in_rust_block {
                assert!(
                    !line.starts_with("# "),
                    "{path}:{} hides required Rust syntax from README readers",
                    index + 1
                );
            }
        }
    }
}
