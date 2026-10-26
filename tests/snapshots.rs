use std::process::Command;

/// Which theme/color configuration a snapshot is rendered with.
#[derive(Clone, Copy)]
enum Mode {
    /// The default UI and syntax themes.
    Default,
    /// The pre-0.1.15 defaults, pinned by name: `--ui-theme clay
    /// --theme base16-ocean.dark`. These snapshots must stay byte-identical
    /// to the output those defaults produced before the default changed.
    Clay,
    /// `NO_COLOR=1`: no escape sequences at all.
    NoColor,
}

fn run_mdx(file: &str, width: u16, mode: Mode) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mdx"));
    // An empty config dir keeps the developer's own config.toml and themes
    // out of the snapshots.
    cmd.env(
        "XDG_CONFIG_HOME",
        std::env::temp_dir().join("mdx_snapshots_no_config"),
    )
    .env_remove("NO_COLOR")
    .arg(file)
    .arg("--no-pager")
    .arg("--width")
    .arg(width.to_string());
    match mode {
        Mode::Default => {}
        Mode::Clay => {
            cmd.args(["--ui-theme", "clay", "--theme", "base16-ocean.dark"]);
        }
        Mode::NoColor => {
            cmd.env("NO_COLOR", "1");
        }
    }
    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("failed to run mdx on {file}: {e}"));
    assert!(
        output.status.success(),
        "mdx failed on {}: {}",
        file,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("non-utf8 output");
    if matches!(mode, Mode::NoColor) {
        assert!(
            !stdout.contains('\x1b'),
            "NO_COLOR output for {file} contains escape sequences"
        );
    }
    stdout
}

macro_rules! snapshot_test {
    ($name:ident, $file:expr, $width:expr) => {
        snapshot_test!($name, $file, $width, Mode::Default);
    };
    ($name:ident, $file:expr, $width:expr, $mode:expr) => {
        #[test]
        fn $name() {
            let output = run_mdx($file, $width, $mode);
            insta::assert_snapshot!(output);
        }
    };
}

// basic.md
snapshot_test!(snapshot_basic_w80, "docs/examples/basic.md", 80);
snapshot_test!(snapshot_basic_w120, "docs/examples/basic.md", 120);

// flowchart-simple.md
snapshot_test!(
    snapshot_flowchart_simple_w80,
    "docs/examples/flowchart-simple.md",
    80
);
snapshot_test!(
    snapshot_flowchart_simple_w120,
    "docs/examples/flowchart-simple.md",
    120
);

// flowchart-advanced.md
snapshot_test!(
    snapshot_flowchart_advanced_w80,
    "docs/examples/flowchart-advanced.md",
    80
);
snapshot_test!(
    snapshot_flowchart_advanced_w120,
    "docs/examples/flowchart-advanced.md",
    120
);

// flowchart-subgraph.md — exercises subgraph/end, quoted labels, <br/>,
// cylinder nodes, extended dotted edges, and quoted edge labels
snapshot_test!(
    snapshot_flowchart_subgraph_w80,
    "docs/examples/flowchart-subgraph.md",
    80
);
snapshot_test!(
    snapshot_flowchart_subgraph_w120,
    "docs/examples/flowchart-subgraph.md",
    120
);

// mixed-content.md
snapshot_test!(
    snapshot_mixed_content_w80,
    "docs/examples/mixed-content.md",
    80
);
snapshot_test!(
    snapshot_mixed_content_w120,
    "docs/examples/mixed-content.md",
    120
);

// syntax-highlight.md
snapshot_test!(
    snapshot_syntax_highlight_w80,
    "docs/examples/syntax-highlight.md",
    80
);
snapshot_test!(
    snapshot_syntax_highlight_w120,
    "docs/examples/syntax-highlight.md",
    120
);

// test-seq-basic.md
snapshot_test!(
    snapshot_seq_basic_w80,
    "docs/examples/test-seq-basic.md",
    80
);
snapshot_test!(
    snapshot_seq_basic_w120,
    "docs/examples/test-seq-basic.md",
    120
);

// test-seq-complex.md
snapshot_test!(
    snapshot_seq_complex_w80,
    "docs/examples/test-seq-complex.md",
    80
);
snapshot_test!(
    snapshot_seq_complex_w120,
    "docs/examples/test-seq-complex.md",
    120
);

// er-minimal.md
snapshot_test!(snapshot_er_minimal_w80, "docs/examples/er-minimal.md", 80);
snapshot_test!(snapshot_er_minimal_w120, "docs/examples/er-minimal.md", 120);

// er-identifying.md
snapshot_test!(
    snapshot_er_identifying_w80,
    "docs/examples/er-identifying.md",
    80
);
snapshot_test!(
    snapshot_er_identifying_w120,
    "docs/examples/er-identifying.md",
    120
);

// er-full.md
snapshot_test!(snapshot_er_full_w120, "docs/examples/er-full.md", 120);
snapshot_test!(snapshot_er_full_w200, "docs/examples/er-full.md", 200);

// er-styled.md
snapshot_test!(snapshot_er_styled_w120, "docs/examples/er-styled.md", 120);

// Unicode fixtures — labels sized by display width (issue #3)
snapshot_test!(
    snapshot_flowchart_unicode_w80,
    "docs/examples/flowchart-unicode.md",
    80
);
snapshot_test!(
    snapshot_flowchart_unicode_w120,
    "docs/examples/flowchart-unicode.md",
    120
);
snapshot_test!(snapshot_er_unicode_w120, "docs/examples/er-unicode.md", 120);
snapshot_test!(
    snapshot_seq_unicode_w120,
    "docs/examples/test-seq-unicode.md",
    120
);

// Math source fallback (issue #13)
snapshot_test!(snapshot_math_w80, "docs/examples/math.md", 80);

// ─── Previous defaults pinned by name (issue #20) ─────────────────────────
// Recorded before the default changed to nord; selecting the old themes by
// name must keep producing exactly this output.

snapshot_test!(clay_basic_w80, "docs/examples/basic.md", 80, Mode::Clay);
snapshot_test!(clay_basic_w120, "docs/examples/basic.md", 120, Mode::Clay);
snapshot_test!(
    clay_flowchart_simple_w80,
    "docs/examples/flowchart-simple.md",
    80,
    Mode::Clay
);
snapshot_test!(
    clay_flowchart_simple_w120,
    "docs/examples/flowchart-simple.md",
    120,
    Mode::Clay
);
snapshot_test!(
    clay_flowchart_advanced_w80,
    "docs/examples/flowchart-advanced.md",
    80,
    Mode::Clay
);
snapshot_test!(
    clay_flowchart_advanced_w120,
    "docs/examples/flowchart-advanced.md",
    120,
    Mode::Clay
);
snapshot_test!(
    clay_flowchart_subgraph_w80,
    "docs/examples/flowchart-subgraph.md",
    80,
    Mode::Clay
);
snapshot_test!(
    clay_flowchart_subgraph_w120,
    "docs/examples/flowchart-subgraph.md",
    120,
    Mode::Clay
);
snapshot_test!(
    clay_mixed_content_w80,
    "docs/examples/mixed-content.md",
    80,
    Mode::Clay
);
snapshot_test!(
    clay_mixed_content_w120,
    "docs/examples/mixed-content.md",
    120,
    Mode::Clay
);
snapshot_test!(
    clay_syntax_highlight_w80,
    "docs/examples/syntax-highlight.md",
    80,
    Mode::Clay
);
snapshot_test!(
    clay_syntax_highlight_w120,
    "docs/examples/syntax-highlight.md",
    120,
    Mode::Clay
);
snapshot_test!(
    clay_seq_basic_w80,
    "docs/examples/test-seq-basic.md",
    80,
    Mode::Clay
);
snapshot_test!(
    clay_seq_basic_w120,
    "docs/examples/test-seq-basic.md",
    120,
    Mode::Clay
);
snapshot_test!(
    clay_seq_complex_w80,
    "docs/examples/test-seq-complex.md",
    80,
    Mode::Clay
);
snapshot_test!(
    clay_seq_complex_w120,
    "docs/examples/test-seq-complex.md",
    120,
    Mode::Clay
);
snapshot_test!(
    clay_er_minimal_w80,
    "docs/examples/er-minimal.md",
    80,
    Mode::Clay
);
snapshot_test!(
    clay_er_minimal_w120,
    "docs/examples/er-minimal.md",
    120,
    Mode::Clay
);
snapshot_test!(
    clay_er_identifying_w80,
    "docs/examples/er-identifying.md",
    80,
    Mode::Clay
);
snapshot_test!(
    clay_er_identifying_w120,
    "docs/examples/er-identifying.md",
    120,
    Mode::Clay
);
snapshot_test!(
    clay_er_full_w120,
    "docs/examples/er-full.md",
    120,
    Mode::Clay
);
snapshot_test!(
    clay_er_full_w200,
    "docs/examples/er-full.md",
    200,
    Mode::Clay
);
snapshot_test!(
    clay_er_styled_w120,
    "docs/examples/er-styled.md",
    120,
    Mode::Clay
);
snapshot_test!(
    clay_flowchart_unicode_w80,
    "docs/examples/flowchart-unicode.md",
    80,
    Mode::Clay
);
snapshot_test!(
    clay_flowchart_unicode_w120,
    "docs/examples/flowchart-unicode.md",
    120,
    Mode::Clay
);
snapshot_test!(
    clay_er_unicode_w120,
    "docs/examples/er-unicode.md",
    120,
    Mode::Clay
);
snapshot_test!(
    clay_seq_unicode_w120,
    "docs/examples/test-seq-unicode.md",
    120,
    Mode::Clay
);
snapshot_test!(clay_math_w80, "docs/examples/math.md", 80, Mode::Clay);

// ─── NO_COLOR: plain text, no escape sequences ────────────────────────────

snapshot_test!(
    no_color_basic_w80,
    "docs/examples/basic.md",
    80,
    Mode::NoColor
);
snapshot_test!(
    no_color_flowchart_simple_w80,
    "docs/examples/flowchart-simple.md",
    80,
    Mode::NoColor
);
snapshot_test!(
    no_color_flowchart_advanced_w80,
    "docs/examples/flowchart-advanced.md",
    80,
    Mode::NoColor
);
snapshot_test!(
    no_color_flowchart_subgraph_w80,
    "docs/examples/flowchart-subgraph.md",
    80,
    Mode::NoColor
);
snapshot_test!(
    no_color_mixed_content_w80,
    "docs/examples/mixed-content.md",
    80,
    Mode::NoColor
);
snapshot_test!(
    no_color_syntax_highlight_w80,
    "docs/examples/syntax-highlight.md",
    80,
    Mode::NoColor
);
snapshot_test!(
    no_color_seq_basic_w80,
    "docs/examples/test-seq-basic.md",
    80,
    Mode::NoColor
);
snapshot_test!(
    no_color_seq_complex_w80,
    "docs/examples/test-seq-complex.md",
    80,
    Mode::NoColor
);
snapshot_test!(
    no_color_er_minimal_w80,
    "docs/examples/er-minimal.md",
    80,
    Mode::NoColor
);
snapshot_test!(
    no_color_er_identifying_w80,
    "docs/examples/er-identifying.md",
    80,
    Mode::NoColor
);
snapshot_test!(
    no_color_er_full_w120,
    "docs/examples/er-full.md",
    120,
    Mode::NoColor
);
snapshot_test!(
    no_color_er_styled_w120,
    "docs/examples/er-styled.md",
    120,
    Mode::NoColor
);
snapshot_test!(
    no_color_flowchart_unicode_w80,
    "docs/examples/flowchart-unicode.md",
    80,
    Mode::NoColor
);
snapshot_test!(
    no_color_er_unicode_w120,
    "docs/examples/er-unicode.md",
    120,
    Mode::NoColor
);
snapshot_test!(
    no_color_seq_unicode_w120,
    "docs/examples/test-seq-unicode.md",
    120,
    Mode::NoColor
);
snapshot_test!(
    no_color_math_w80,
    "docs/examples/math.md",
    80,
    Mode::NoColor
);
