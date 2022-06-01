//run: cargo test -- --nocapture

#![allow(dead_code)]

#[macro_use]
mod framework;
mod ast;
mod errors;
mod lexer;
mod sexpr;
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
    use super::*;
    use framework::{self, Token};
    use std::fmt::Debug;

    const FILE: &str = r#"
:title: Hello
{# Comment #}
:bibliography: {$ env "BIBLIOGRAPHY" $}
{# | test_pipe  #}

{| run "graphviz" hello | prettify .  |}
digraph {
    A -> B
    A -> C
    {$ include "nodes" $}
}
{| end |}

== Lorem
Some text

{$ "This is a quote that\nshould be included" $}

{| if(nottrue) |}
Come to the dark side of the moon
{| endif |}

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
        let (ast, args) = log(FILE, ast::process(&sexprs, &args, FILE));
        //ast.iter()
        //    .enumerate()
        //    .for_each(|(i, t)| println!("{} -> {}", t.to_display(&args, FILE), i));
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
