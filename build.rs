use std::env;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let packdump_path = Path::new(&out_dir).join("syntaxes.packdump");

    // Start with syntect's bundled defaults
    let mut builder = syntect::parsing::SyntaxSet::load_defaults_newlines().into_builder();

    // Add our custom grammars on top (later additions override defaults for same-name syntaxes)
    builder
        .add_from_folder("syntaxes", true)
        .expect("Failed to load custom syntaxes from syntaxes/ directory");

    let syntax_set = builder.build();

    syntect::dumps::dump_to_uncompressed_file(&syntax_set, &packdump_path)
        .expect("Failed to write syntax packdump");

    // Rebuild if any syntax file changes
    println!("cargo:rerun-if-changed=syntaxes");
}
