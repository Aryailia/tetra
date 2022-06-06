//run: cargo test -- --nocapture

#![allow(dead_code)]
#![feature(let_chains)]

#[macro_use]
mod framework;
mod errors;
mod parser;
mod run;

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
    use parser::{ast, lexer, sexpr};

    const FILE: &str = r#"
:title: Hello
{# Comment #}
:bibliography: {$ hello = env "yo"; env "BIBLIOGRAPHY" $}
{# | test_pipe  #}

{| run "graphviz" hello | prettify . ; env . |}
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
{| endif; cite(cite hello) |}

Final stuff
"#;

    #[test]
    #[allow(dead_code)]
    fn it_works() {
        let _function_list = ["hello", "cite"];
        let lexemes = log(FILE, lexer::process(FILE, true));
        //lexemes.iter().for_each(|l| println!("{:?} {:?}", l, l.to_str(FILE)));
        let (sexprs, args) = log(FILE, sexpr::process(&lexemes, FILE));
        //sexprs
        //    .iter()
        //    .enumerate()
        //    .for_each(|(i, s)| println!("{:<3} {}", i, s.to_display(&args, FILE)));
        let (ast, args, provides_for) = log(FILE, ast::process(&sexprs, &args));
        ast.iter().enumerate().for_each(|(i, t)| {
            println!(
                "{:?} | {} -> {}",
                &provides_for[t.provides_for.0..t.provides_for.1],
                t.to_display(&args, FILE),
                i
            )
        });
        log(FILE, run::run(&ast, &args, &_function_list, FILE));
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
