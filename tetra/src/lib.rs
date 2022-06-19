//run: cargo test -- --nocapture

#![allow(dead_code)]
#![feature(let_chains)]

#[macro_use]
mod framework;
mod errors;
pub mod parser;
pub mod run;

pub use framework::Token;

//use std::fmt::Debug;
//
//#[allow(dead_code)]
//fn log<T, E: Debug>(original: &str, result: Result<T, framework::Token<E>>) -> T {
//    match result {
//        Ok(s) => s,
//        Err(e) => {
//            eprintln!("{} {:?}", e.get_context(original), e);
//            std::process::exit(1);
//            //panic!("\n{:?}\n{}", e, e.get_context(original));
//        }
//        //Err(e) => match e {
//        //    CustomErr::Parse(err) => panic!("\nERROR: {:?}\n", err.msg()),
//        //    err => panic!("{:?}", err),
//        //},
//    }
//}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use super::*;
    use framework::{self, Token};

    //#[test]
    #[allow(dead_code)]
    fn it_works() {
        let file = _REF;
        let lexemes = log(file, parser::step1_lex(file, true));
        //lexemes.iter().for_each(|l| println!("{:?} {:?}", l, l.to_str(file)));
        let (sexprs, args) = log(file, parser::step2_to_sexpr(&lexemes, file));
        //sexprs
        //    .iter()
        //    .enumerate()
        //    .for_each(|(i, s)| println!("{:<3} {}", i, s.to_display(&args, file)));
        let (ast, args, provides_for) = log(file, parser::step3_to_ast(&sexprs, &args));
        ast.iter().enumerate().for_each(|(i, t)| {
            println!(
                "{:?} | {} -> {}",
                &provides_for[t.provides_for.0..t.provides_for.1],
                t.to_display(&args, file),
                i
            )
        });
        let ctx = run::markup::default_context();
        let out = log(file, ctx.run(&ast, &args, file));
        //println!("{}", out);

        if false {
            use std::io::Write;
            let mut outfile = std::fs::File::create(
                std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/test.adoc"))
            ).unwrap();
            writeln!(&mut outfile, "{}", out).unwrap();
        }
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

    const _FILE: &str = r#"
:title: Hello
{# Comment #}
:bibliography: {$ hello = env "HOME"; env "BIBLIOGRAPHY" $}
{# | test_pipe  #}

{| run "graphviz", hello | prettify . |}
digraph {
    A -> B
    A -> C
    {$ include "nodes" $}
}
{| end |}

== Lorem
Some text

{$ "This is a quote that\nshould be included" $}

{| if(hello) |}
Come to the dark side of the moon
{| endif; cite(hello, hello, "a") |}

Final stuff
"#;

    const _REF: &str = r#"
{$ cite "@capper2012" $} the
{$ cite "@margulis2004" $} quick brown
{$ cite "[@steinfieldEtAl2012]" $} do

+++++
{| run "graphviz" |}
digraph {
    A -> B
    A -> C
}
{| end |}
++++

This is an example of sh

{| run "sh" |}
echo yo
{| endif |}


== References

{$ references $}

stuff

"#;
}
