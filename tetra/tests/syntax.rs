//run: cargo test -- --nocapture

macro_rules! compare_eq {
    ($ctx:ident, $( $source:literal => $answer:literal )*) => {
        $( assert_eq!($ctx.compile($source).as_ref().map(String::as_str), Ok($answer)); )*
    };
}

#[test]
fn edge_cases() {
    let ctx = tetra::run::markup::default_context();
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
