//run: cargo test -- --nocapture

macro_rules! re_export {
    ($mod:ident) => {
        mod $mod; pub use $mod::*;
    };
}

re_export!(asciidoctor);
re_export!(commonmark);
