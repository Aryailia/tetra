//run: cargo test -- --nocapture

#![allow(dead_code)]

#[macro_use]
mod framework;
mod lexer;
mod parser;
mod errors;

#[cfg(test)]
mod tests {
    use super::*;
    use framework::{self, Token};
    use std::fmt::Debug;

    const FILE: &str = r#"
:title: Hello
:bibliography:{| pandoc | cite (narrative (at_hello . )) |}

== Lorem

This must be a list

{# because of this comment #}

Meep

{$ "This is a quote that\nshould be included" $}

{| if (nottrue) |}


Come to the dark side of the moon

{| endif |}

"#;

    #[test]
    #[allow(dead_code)]
    fn it_works() {
        log(FILE, lexer::process(FILE, true));
        return;
    }

    #[allow(dead_code)]
    fn log<T, E: Debug>(original: &str, result: Result<T, Token<E>>) -> T {
        match result {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{} {:?}", e.get_context(original), e);
                std::process::exit(1);
                //panic!("\n{:?}\n{}", e, e.get_context(original));
            }
            //Err(e) => match e {
            //    CustomErr::Parse(err) => panic!("\nERROR: {:?}\n", err.msg()),
            //    err => panic!("{:?}", err),
            //},
        }
    }


}
