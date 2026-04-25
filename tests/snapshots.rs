use std::process::Command;

fn run_mdx(file: &str, width: u16) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg(file)
        .arg("--no-pager")
        .arg("--width")
        .arg(width.to_string())
        .output()
        .unwrap_or_else(|e| panic!("failed to run mdx on {}: {}", file, e));
    assert!(
        output.status.success(),
        "mdx failed on {}: {}",
        file,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("non-utf8 output")
}

macro_rules! snapshot_test {
    ($name:ident, $file:expr, $width:expr) => {
        #[test]
        fn $name() {
            let output = run_mdx($file, $width);
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
