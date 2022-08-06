//run: cargo build
use std::env;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

fn main() {
    // 'enum_dispatch' might be useful
    macro_rules! declare {
        ($enum:ident, $( $variant:ident: $struct:ident $( $ext:expr )*; )*) => {
            let output = &mut String::new();

            //#[derive(Debug)]
            //#[repr(usize)]
            pub enum $enum {
                $( $variant, )*
            }
            writeln!(output, "#[derive(Clone, Copy, Debug)]").unwrap();
            writeln!(output, "#[repr(usize)]").unwrap();
            writeln!(output, "pub enum {} {{", stringify!($enum)).unwrap();
            $( writeln!(output, "    {},", stringify!($variant)).unwrap(); )*
            writeln!(output, "}}").unwrap();
            writeln!(output, "").unwrap();

            //const VTABLE: [&dyn Analyse; 0 $( + { _ = $enum::$variant; 1 } )*] = [
            //    $( &$struct(), )*
            //];
            let count = 0 $(+ { _ = $enum::$variant; 1 } )*;
            writeln!(output, "const VTABLE: [&dyn Analyse; {}] = [", count).unwrap();
            $( writeln!(output, "    &{}(),", stringify!($struct)).unwrap(); )*
            writeln!(output, "];").unwrap();
            writeln!(output, "").unwrap();


            let mut map = phf_codegen::Map::new();
            map $($( .entry($ext, concat!(stringify!($enum), "::", stringify!($variant))) )*)*;
            writeln!(output, "const FROM_EXT: phf::Map<&'static str, FileType> = {};", map.build()).unwrap();

            //panic!("{}", output);

            let path = Path::new(&env::var("OUT_DIR").unwrap()).join("codegen.rs");
            let mut file = BufWriter::new(File::create(&path).unwrap());

            write!(&mut file, "{}", output).unwrap();
        };
    }

    declare! {FileType,
        AsciiDoctor: AsciiDoctor "ad" "adoc" "asc" "asciidoc";
        CommonMark: CommonMark "md" "markdown";
        Markdown: Todo;
        RMarkdown: Todo;

        HTML: Todo;
        LaTeX: Todo;
        PDF: Todo;

        Default: Todo;
    }


    //writeln!(
    //    &mut file,
    //     "static KEYWORDS: phf::Map<&'static str, FileType> = \n{};\n",
    //     phf_codegen::Map::new()
    //         .entry("loop", "FileType::Loop")
    //         .entry("continue", "FileType::Continue")
    //         .entry("break", "FileType::Break")
    //         .entry("fn", "FileType::Fn")
    //         .entry("extern", "FileType::Extern")
    //         .build()
    //).unwrap();
}
