//run: cargo test -- --nocapture

use tetra::api::{FileType, Metadata};

macro_rules! compare_eq {
    ($ctx:ident, $( $source:literal => $answer:literal )*) => {
        let metadata = Metadata::new(FileType::Markdown, FileType::HTML);
        $( assert_eq!(
            $ctx.compile($source, metadata.clone()).as_ref().map(String::as_str),
            Ok($answer)
        ); )*
    };
}

#[test]
fn edge_cases() {
    let ctx = tetra::default_context();
    compare_eq! { ctx,
        ""                  => ""
        "a"                 => "a"
        "{| ; . |} a"       => " a"
        "{| . |} a"         => " a a"
        "{| .; |} a"        => ""

        "{| a = |}b{| a |}" => "bb"
        // TODO: This should report back an error that it is not defined yet
        //"{| ; a = . |} b {$ a $}" => ""
    }
}
