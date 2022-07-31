//run: cargo test -- --nocapture

#![allow(dead_code)]
//#![feature(let_chains)]
//#![feature(arbitrary_enum_discriminant)]

#[macro_use]
mod framework;
mod errors;
pub mod parser;
pub mod run;
pub mod api;

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
    use api::{Metadata, FileType};

    //#[test]
    //fn async_run() {
    //    let file = _FILE;
    //    let lexemes = log(file, parser::step1_lex(file, true));
    //    let (sexprs, args) = log(file, parser::step2_to_sexpr(&lexemes, file));
    //    let (ast, args, _provides_for) = log(file, parser::step3_to_ast(&sexprs, &args));

    //    let rt = tokio::runtime::Runtime::new().unwrap();
    //    let ctx = run::markup::default_context();
    //    rt.block_on(async {
    //        run::exec_async::run(&ctx, &ast, &args, file);
    //    })
    //}

    const _EG: [&str; 4] = [
        // These cannot be run
        LITERAL, // 0, checks if the "{{|" etc. work
        PAREN,   // 1, checks if "cite(" vs "cite (" etc. work

        // These should be runnable by the 'default_context()'
        GENERAL, // 2, a general use case with "run()"
        REF,     // 3, checking "cite()" functionality
    ];

    #[test]
    #[allow(dead_code)]
    fn it_works() {
        let file = _EG[2];
        let lexemes = log(file, parser::step1_lex(file, true));
        //lexemes.iter().for_each(|l| println!("{:?} {:?}", l, l.to_str(file)));
        let sexprs = log(file, parser::step2_to_sexpr(&lexemes, file));
        //sexprs
        //    .0
        //    .iter()
        //    .enumerate()
        //    .for_each(|(i, s)| println!("{:<3} {}", i, s.to_display(&sexprs.1, file)));
        let ast = log(file, parser::step3_to_ast(&sexprs));
        //ast.iter().enumerate().for_each(|(i, t)| {
        //    println!(
        //        "{:?} | {} -> {}",
        //        &provides_for[t.provides_for.0..t.provides_for.1],
        //        t.to_display(&args, file),
        //        i
        //    )
        //});
        let ctx = run::markup::default_context();
        let out = match ctx.run(&ast, Metadata::new(FileType::Markdown, FileType::HTML), file) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        };
        println!("{}", out);

        if false {
            use std::io::Write;
            let mut outfile = std::fs::File::create(
                std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/test.adoc"))
            ).unwrap();
            writeln!(&mut outfile, "{}", out).unwrap();
        }
    }

    #[allow(dead_code)]
    pub fn log<T, E: Debug>(original: &str, result: Result<T, Token<E>>) -> T {
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

    pub const LITERAL: &str = r#"
body 1
{{| not block |}}
{{$ not inline $}}
{{# not comment #}}
Body 2"#;

    pub const PAREN: &str = r#"
body 1
{$ cite "a", "2" $}
{$ cite("a", "2") $}
{$ cite (cite "2") $}
{$ cite(cite("a", "2"), "三") $}
{$ cite("a", cite("2", "三")) $}
{$ cite("a", (cite "2", "三")) $}
{$ r = cite "a", "2" $}
{$ s = cite("a", "2") $}
{$ t = cite(cite("a", "2"), "三") $}
{$ u = cite "a", cite("2", "三") $}
{$ w = v = u $}
"#;
    //{{$ cite("a", cite "2", "三") $}}

    pub const GENERAL: &str = r#"
:title: Hello
{# Comment #}
:bibliography: {$ hello = env "HOME"; env "BIBLIOGRAPHY" $}
{# | test_pipe  #}

{| run "graphviz" | concat . |}
digraph {
  A -> B
  A -> C
  {$ concat "nodes" $}
}
{| end |}

== Lorem
Some text

{$ "This is a quote that\nshould be included" $}

{| if_equals(hello, "") |}
Come to the dark side of the moon
{| end; concat(hello, hello, "a") |}

This should not show up in the output
"#;

    pub const REF: &str = r#"
{$ cite "@capper2012" $} the
{$ cite "@margulis2004" $} quick brown
{$ cite "[@steinfieldEtAl2012]" $} do

++++
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
{| end |}


== References

{$ references $}

stuff"#;
}

#[cfg(not(test))]
//#[cfg(test)]
mod sexpr_tests {
    #[allow(unused_imports)]
    use super::*;

    // NOTE: Does not check the str body of Item::Reference(_)
    #[allow(unused_macros)]
    macro_rules! make_sexpr_test {
        ($original:expr,
            $( $id:literal : $( $item:ident$(($ref:literal))? $val:literal,)* )*
        ) => {
            use parser::Item;
            use parser::SexprOutput;
            let file = $original;
            let lexemes = tests::log(file, parser::step1_lex(file, true));
            let SexprOutput(se_list, args) = tests::log(file, parser::step2_to_sexpr(&lexemes, file));
            let mut sexpr_iter = se_list.iter();
            let mut buffer = String::new();
            let mut id_tracker = 0; // Ensures {$id} always increments by 1

            $({
                let s = sexpr_iter.next().unwrap();
                assert_eq!(
                    id_tracker,
                    $id,
                    "Incorrect id\nAt line: {}: {}\n",
                    $id,
                    stringify!($( $item$(($ref))?, )*),
                );
                id_tracker += 1;
                //assert_eq!(i, $id, "\nDEV: Bad id\nFrom:\n  {:?}\n  {}\n",
                //    Item::$item$(($ref))?,
                //    s.to_display(&args, file),
                //);

                // Check number of arguments is correct
                #[allow(unused_mut)]
                let mut i = s.args.0;
                let arg_count = 0 $( + {$val; 1} )*;
                assert_eq!(
                    arg_count,
                    s.args.1 - s.args.0,

                    "\n\nHas the incorrect number of arguments\nExpect:\n  {}: {}\n  {}\nFrom:\n  {}\n",

                    $id,
                    stringify!($( $item$(($ref))?, )*),
                    args[s.args.0..s.args.1]
                        .iter()
                        .fold(String::new(), |mut buffer, t| {
                            buffer.push_str(&format!("{:?}, ", t.me));
                            buffer
                        }),
                    s.to_display(&args, file),
                );
                $(
                    let arg = &args[i];

                    // Check {arg} is the correct str value
                    buffer.clear();
                    if let Item::Reference(_) = Item::$item$(($ref))? {
                        // Do not check references
                    } else {
                        assert_eq!(
                            $val,
                            arg.to_str(file),
                            "\n    {}: {:?}: {}\n",
                            $id,
                            Item::$item$(($ref))?,
                            s.to_display(&args, file),
                        )
                    }

                    // Check ${arg} is the correct parameter type
                    assert_eq!(
                        Item::$item$(($ref))?,
                        arg.me,
                    );
                    i += 1;
                )*
                assert_eq!(i, s.args.1, "This should always work");
            })*

            assert_eq!(se_list.len(), id_tracker, "Mismatched amount of sexprs\n");
            //for sexpr in sexprs.0 {
            //    for (i, arg) in sexprs.1[sexpr.args.0..sexpr.args.1].iter().enumerate() {
            //        assert!(matches!(arg.me, &ref_args[i]))
            //    }
            //}
        };

    }

    #[test]
    #[allow(dead_code)]
    fn test_literal_sexpr() {
        make_sexpr_test!(tests::LITERAL,
            0: Stdin "",
            1: Str "\nbody 1\n",
               Text("{|") "{{|", Str " not block ",   Text("|}") "|}}", Str "\n",
               Text("{$") "{{$", Str " not inline ",  Text("$}") "$}}", Str "\n",
               Text("{#") "{{#", Str " not comment ", Text("#}") "#}}",
               Str "\nBody 2",
            2: Reference(0) "",
        );
    }

    #[test]
    #[allow(dead_code)]
    fn test_paren_sexpr() {
        make_sexpr_test!(tests::PAREN,
            0:  Stdin "",
            1:  Str "a", 2:  Str "2",
            3:  Ident "cite", Reference(1) "", Reference(2) "",

            4:  Str "a", 5:  Str "2",
            6:  Func "cite", Reference(4) "", Reference(5) "",
            7:  Reference(6) "",

            8:  Str "2",
            9:  Ident "cite", Reference(8) "",
            10: Ident "cite", Reference(9) "",

            11: Str "a", 12: Str "2",
            13: Func "cite", Reference(11) "", Reference(12) "",
            14: Str "三",
            15: Func "cite", Reference(13) "", Reference(14) "",
            16: Reference(15) "",

            17: Str "a", 18: Str "2", 19: Str "三",
            20: Func "cite", Reference(18) "", Reference(19) "",
            21: Func "cite", Reference(17) "", Reference(20) "",
            22: Reference(21) "",

            23: Str "a", 24: Str "2", 25: Str "三",
            26: Ident "cite", Reference(24) "", Reference(25) "",
            27: Func "cite", Reference(23) "", Reference(26) "",
            28: Reference(27) "",

            29: Str "a", 30: Str "2",
            31: Ident "cite", Reference(29) "", Reference(30) "",
            32: Assign "=", Ident "r", Reference(31) "",

            33: Str "a", 34: Str "2",
            35: Func "cite", Reference(33) "", Reference(34) "",
            36: Reference(35) "",
            37: Assign "=", Ident "s", Reference(36) "",

            38: Str "a", 39: Str "2",
            40: Func "cite", Reference(38) "", Reference(39) "",
            41: Str "三",
            42: Func "cite", Reference(40) "", Reference(41) "",
            43: Reference(42) "",
            44: Assign "=", Ident "t", Reference(43) "",

            45: Str "a", 46: Str "2", 47: Str "三",
            48: Func "cite", Reference(46) "", Reference(47) "",
            49: Ident "cite", Reference(45) "", Reference(48) "",
            50: Assign "=", Ident "u", Reference(49) "",

            51: Ident "u",
            52: Assign "=", Ident "v", Reference(51) "",
            53: Assign "=", Ident "w", Reference(52) "",
            54: Str "\nbody 1\n",
                Reference(3) "",  Str "\n",
                Reference(7) "",  Str "\n",
                Reference(10) "", Str "\n",
                Reference(16) "", Str "\n",
                Reference(22) "", Str "\n",
                Reference(28) "", Str "\n",
                Reference(32) "", Str "\n",
                Reference(37) "", Str "\n",
                Reference(44) "", Str "\n",
                Reference(50) "", Str "\n",
                Reference(53) "", Str "\n",
            55: Reference(0) "", // For the knit
        );
    }

    #[test]
    #[allow(dead_code)]
    fn test_general_sexpr() {
        make_sexpr_test!(tests::GENERAL,
            0:  Stdin "",
            1:  Str "HOME",
            2:  Ident "env", Reference(1) "",
            3:  Assign "=", Ident "hello", Reference(2) "",
            4:  Str "BIBLIOGRAPHY",
            5:  Ident "env", Reference(4) "",
            6:  Str "\n:title: Hello\n", Str "\n:bibliography: ",
                Reference(5) "", Str "\n", Str "\n\n",
            7:  Str "graphviz",
            8:  Ident "run", Reference(7) "", Stdin "{|",
            9:  Ident "concat", Stdin ".", Reference(8) "",
            10: Str "nodes",
            11: Ident "concat", Reference(10) "",
            12: Str "\ndigraph {\n  A -> B\n  A -> C\n  ",
                Reference(11) "", Str "\n}\n",
            13: Ident "end", Stdin "{|",
            14: Str "This is a quote that", Text("\n") "\\n", Str "should be included",
            15: Reference(14) "",
            16: Str "\n\n== Lorem\nSome text\n\n", Reference(15) "", Str "\n\n",
            17:
            18: Ident "hello",
            19: Func "if_equals", Reference(18) "", Reference(17) "", Stdin "{|",
            20: Reference(19) "",
            21: Str "\nCome to the dark side of the moon\n",
            22: Ident "end", Stdin "{|",
            23: Str "a",
            24: Ident "hello",
            25: Ident "hello",
            26: Func "concat", Reference(24) "", Reference(25) "", Reference(23) "",
            27: Reference(26) "",
            28: Str "\n\nThis should not show up in the output\n",
            29: Reference(0) "", Reference(9) "", Reference(13) "",
                Reference(20) "", Reference(27) "",
        );
    }

    #[test]
    #[allow(dead_code)]
    fn test_ref_sexpr() {
        make_sexpr_test!(tests::REF,
            0:  Stdin "",
            1:  Str "@capper2012",
            2:  Ident "cite", Reference(1) "",
            3:  Str "@margulis2004",
            4:  Ident "cite", Reference(3) "",
            5:  Str "[@steinfieldEtAl2012]",
            6:  Ident "cite", Reference(5) "",
            7:  Str "\n",
                Reference(2) "", Str " the\n",
                Reference(4) "", Str " quick brown\n",
                Reference(6) "", Str " do\n\n++++\n",
            8:  Str "graphviz",
            9:  Ident "run", Reference(8) "", Stdin "{|",
            10: Str "\ndigraph {\n  A -> B\n  A -> C\n}\n",
            11: Ident "end", Stdin "{|",
            12: Str "\n++++\n\nThis is an example of sh\n\n",
            13: Str "sh",
            14: Ident "run", Reference(13) "", Stdin "{|",
            15: Str "\necho yo\n",
            16: Ident "end", Stdin "{|",
            17: Ident "references",
            18: Str "\n\n\n== References\n\n", Reference(17) "", Str "\n\nstuff",
            19: Reference(0) "", Reference(9) "", Reference(11) "",
                Reference(14) "", Reference(16) "",
        );
    }

}
