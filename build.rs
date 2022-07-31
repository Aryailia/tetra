//run: cargo test -- --nocapture
use tetra::{
    api::{FileType, Metadata},
    run::markup,
};

use std::env;
use std::fs;
use std::path::Path;

const README_SOURCE: &str = "readme-source.md";
const README: &str = "README.md";

// https://doc.rust-lang.org/cargo/reference/build-scripts.html

fn main() {
    //println!("cargo:rerun-if-changed=src/hello.c");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let project_path = Path::new(&manifest_dir);

    let readme_source = project_path.join(README_SOURCE);
    let buffer = match fs::read_to_string(&readme_source) {
        Ok(s) => s,
        Err(err) => {
            //eprintln!("build.rs: {:?}: {}", err),
            panic!("Missing {:?} file. {}", readme_source, err);
        }
    };

    let ctx = markup::default_context();
    let output = ctx.compile(&buffer, Metadata::new(FileType::Markdown, FileType::Markdown)).unwrap();

    let output_path = Path::new(&project_path).join(README);
    fs::write(output_path, output.as_bytes()).unwrap();
}

//fn build_lang_file() {
//}
