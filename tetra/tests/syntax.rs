//run: cargo test -- --nocapture

use std::fmt::Display;

use tetra::{parser, Token};

fn unwrap<T, E: Display>(original: &str, result: Result<T, Token<E>>) -> T {
    match result {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{} {}", e.get_context(original), e.me);
            std::process::exit(1);
        }
    }
}

fn process(content: &str) -> String {
    //tetra::parse(content);
    let ctx = tetra::run::markup::default_context();

    let lexemes = unwrap(content, parser::step1_lex(content, true));
    //lexemes.iter().for_each(|l| println!("{:?} {:?}", l, l.to_str(content)));

    let sexprs = unwrap(content, parser::step2_to_sexpr(&lexemes, content));
    //sexprs.0
    //    .iter()
    //    .enumerate()
    //    .for_each(|(i, s)| println!("{:<3} {}", i, s.to_display(&sexprs.1, content)));

    let (ast, args, _provides_for) = unwrap(content, parser::step3_to_ast(&sexprs));
    //ast.iter().enumerate().for_each(|(i, t)| {
    //        println!(
    //            "{:?} | {} -> {}",
    //            &_provides_for[t.provides_for.0..t.provides_for.1],
    //            t.to_display(&args, content),
    //            i
    //        )
    //    });

    //ast.iter().enumerate().for_each(|(i, t)| {
    //    println!(
    //        "{:?} | {} -> {}",
    //        &provides_for[t.provides_for.0..t.provides_for.1],
    //        t.to_display(&args, file),
    //        i
    //    )
    //});

    match ctx.run(&ast, &args, content) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    }
    //"".to_string()
}

#[test]
fn edge_cases() {
    //let ctx = tetra::run::markup::default_context();
    //assert!(ctx.compile("") ==  Ok("".to_string()));

    assert_eq!(process(""), "");
    assert_eq!(process("a"), "a");
    assert_eq!(process("{| ; . |} a"), " a");
    assert_eq!(process("{| . |} a"), " a a");
    assert_eq!(process("{| .; |} a"), "");

    assert_eq!(process("{| a = |} b {| a |}"), " b  b ");
    // TODO: This should report back an error that it is not defined yet
    //assert_eq!(process("{| ; a = . |} b {$ a $}"), "");
}

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
