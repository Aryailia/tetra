//run: cargo test -- --nocapture


#[cfg(test)]
mod tests {
    use tetra::api::{FileType, Config};

    macro_rules! compare_eq {
        ($ctx:ident, $( $source:literal => $answer:literal )*) => {
            let config = Config::new(FileType::Markdown, FileType::Html);
            $( assert_eq!(
                $ctx.compile($source, config.clone()).as_ref().map(String::as_str),
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
}
