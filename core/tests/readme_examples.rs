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

#[test]
fn root_readme_keeps_each_crate_badged_and_separate() {
    let readme = READMES[0].1;
    for (name, path) in [
        ("tracing-layer-slack", "layers/slack"),
        ("tracing-layer-discord", "layers/discord"),
        ("tracing-layer-core", "core"),
    ] {
        let badge_name = name.replace('-', "--");
        assert!(
            readme.contains(&format!(
                "- [![{name}](https://img.shields.io/badge/{badge_name}-blue)]({path})"
            )),
            "README.md does not give {} its own badge row",
            name
        );
        assert!(
            readme.contains(&format!(
                "[![{name} on crates.io](https://img.shields.io/crates/v/{name}.svg)](https://crates.io/crates/{name})"
            )),
            "README.md does not show the current {} version",
            name
        );
        assert!(
            readme.contains(&format!(
                "[![{name} documentation](https://docs.rs/{name}/badge.svg)](https://docs.rs/{name})"
            )),
            "README.md does not show the {} documentation badge",
            name
        );
        assert!(
            readme.contains(&format!(
                "[![{name} downloads](https://img.shields.io/crates/d/{name})](https://crates.io/crates/{name})"
            )),
            "README.md does not show {} downloads",
            name
        );
        assert!(
            readme.contains(&format!("## {name}\n")),
            "README.md does not give {} its own section",
            name
        );
    }
}

#[test]
fn platform_readmes_keep_their_crate_badges() {
    for (path, readme, name) in [
        (READMES[1].0, READMES[1].1, "tracing-layer-discord"),
        (READMES[2].0, READMES[2].1, "tracing-layer-slack"),
    ] {
        assert!(
            readme.contains(&format!(
                "[![{name} on crates.io](https://img.shields.io/crates/v/{name}.svg)](https://crates.io/crates/{name})"
            )),
            "{} does not show the current {} version",
            path,
            name
        );
        assert!(
            readme.contains(&format!(
                "[![{name} documentation](https://docs.rs/{name}/badge.svg)](https://docs.rs/{name})"
            )),
            "{} does not show the {} documentation badge",
            path,
            name
        );
    }
}
