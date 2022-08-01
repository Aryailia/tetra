//run: cargo test -- --nocapture

#![allow(dead_code)]
//#![feature(let_chains)]
//#![feature(arbitrary_enum_discriminant)]

#[macro_use]
mod framework;
mod errors;
pub mod parser;
#[macro_use]
pub mod run;
pub mod api;
mod default_markup;

pub use default_markup::default_context;
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
    use api::{FileType, Metadata};
    use framework::{self, Token};

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
        // These should be runnable by the 'default_context()'
        GENERAL, // 0, a general use case with "run()"
        REF,     // 1, checking "cite()" functionality
        // These cannot be run
        LITERAL, // 2, checks if the "{{|" etc. work
        PAREN,   // 3, checks if "cite(" vs "cite (" etc. work
    ];

    #[test]
    #[allow(dead_code, unreachable_code)]
    fn it_works() {
        let file = _EG[0]; //r#"{$ cite "@margulis2004", env: bib $}"#;
        let lexemes = log(file, parser::step1_lex(file, true));
        //lexemes.iter().for_each(|l| println!("{:?} {:?}", l, l.to_str(file)));
        let sexprs = log(file, parser::step2_to_sexpr(&lexemes, file));
        //sexprs
        //    .0
        //    .iter()
        //    .enumerate()
        //    .for_each(|(i, s)| println!("{:<3} {}", i, s.to_display(&sexprs.1, file)));
        let ast = log(file, parser::step3_to_ast(&sexprs, file));
        //ast.0.iter().enumerate().for_each(|(i, t)| {
        //    println!(
        //        "{:?} | {} -> {}",
        //        &ast.2[t.provides_for.0..t.provides_for.1],
        //        t.to_display(&ast.1, file),
        //        i
        //    )
        //});
        //std::process::exit(0);
        let ctx = default_context();
        let out = match ctx.run(
            &ast,
            Metadata::new(FileType::Markdown, FileType::AsciiDoctor),
            file,
        ) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        };
        println!("{}", out);

        if false {
            use std::io::Write;
            let mut outfile = std::fs::File::create(std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/test.adoc"
            )))
            .unwrap();
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

    //{{$ cite("a", cite "2", "三") $}}

    pub const GENERAL: &str = r#"
:title: Hello
{| ; hello = env "HOME"; concat(":author:       ", hello, .) |}
:home:         {$ hello $}
:bibliography: {$ env "BIBLIOGRAPHY" $}
{# Comment  #}

++++
{| run "graphviz" | concat . |}
digraph {
  A -> B
  A -> C
  {$ concat "nodes" $}
}
{| end |}
++++

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

This is an example of sh

{| run "sh" |}
echo yo
{| end |}


== References

{$ references $}

stuff"#;

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
}

//#[cfg(not(test))]
#[cfg(test)]
mod sexpr_tests {
    #[allow(unused_imports)]
    use super::*;

    // NOTE: Does not check the str body of Item::Reference(_)
    #[allow(unused_macros)]
    macro_rules! make_sexpr_test {
        ($original:expr,
            $( $id:literal : $head_ty:ident $head_val:literal |
                $( $item:ident$(($ref:literal))? $val:literal,)* )*
        ) => {
            use parser::{Item, Label};
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

                // Check the s-expr head
                assert_eq!(
                    $head_val,
                    s.head.to_str(file),
                    "\n    {}: {:?}: {}\n",
                    $id,
                    Label::$head_ty,
                    s.to_display(&args, file),
                );

                // Check ${arg} is the correct parameter type
                assert_eq!(
                    Label::$head_ty,
                    s.head.me,
                    "\nFrom: {} {}\nHead is incorrect type\n",
                    $id,
                    $head_val
                );


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
                        "\nFrom: {} {:?}\nIncorrect type\n",
                        $id,
                        Item::$item$(($ref))?,
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
    fn test_general_sexpr() {
        make_sexpr_test!(tests::GENERAL,
            0:  Concat ""| Stdin "",
            1:  Concat ""| Str "\n:title: Hello\n",
            2:  Concat ""| Stdin "{|",
            3:  Concat ""| Str "HOME",
            4:  Ident "env"| Reference(3) "",
            5:  Assign "="| Ident "hello", Reference(4) "",
            6:  Concat ""| Str ":author:       ",
            7:  Ident "hello"|
            8:  Func "concat"| Reference(6) "", Reference(7) "", Stdin ".",
            9:  Concat ""| Reference(8) "",
            10: Ident "hello"|
            11: Concat ""| Str "BIBLIOGRAPHY",
            12: Ident "env"| Reference(11) "",
            13: Concat ""| Str "\n:home:         ", Reference(10) "",
                           Str "\n:bibliography: ", Reference(12) "", Str "\n",
                           Str "\n\n++++\n",
            14: Concat ""| Str "graphviz",
            15: Ident "run"| Reference(14) "", Stdin "{|",
            16: Ident "concat"| Stdin ".", Reference(15) "",
            17: Concat ""| Str "nodes",
            18: Ident "concat"| Reference(17) "",
            19: Concat ""| Str "\ndigraph {\n  A -> B\n  A -> C\n  ",
                           Reference(18) "", Str "\n}\n",
            20: Ident "end"| Stdin "{|",
            21: Concat ""| Str "This is a quote that", Literal("\n") "\\n",
                           Str "should be included",
            22: Concat ""| Reference(21) "",
            23: Concat ""| Str "\n++++\n\n== Lorem\nSome text\n\n",
                           Reference(22) "", Str "\n\n",
            24: Concat ""| Literal("") "\"\"",
            25: Ident "hello"|
            26: Func "if_equals"| Reference(25) "", Reference(24) "", Stdin "{|",
            27: Concat ""| Reference(26) "",
            28: Concat ""| Str "\nCome to the dark side of the moon\n",
            29: Ident "end"| Stdin "{|",
            30: Concat ""| Str "a",
            31: Ident "hello"|
            32: Ident "hello"|
            33: Func "concat"| Reference(31) "", Reference(32) "", Reference(30) "",
            34: Concat ""| Reference(33) "",
            35: Concat ""| Str "\n\nThis should not show up in the output\n",
            36: Concat ""| Reference(0) "", Reference(9) "", Reference(16) "",
                           Reference(20) "", Reference(27) "", Reference(34) "",
        );
    }

    #[test]
    #[allow(dead_code)]
    fn test_ref_sexpr() {
        make_sexpr_test!(tests::REF,
            0:  Concat ""| Stdin "",
            1:  Concat ""| Str "@capper2012",
            2:  Ident "cite"| Reference(1) "",
            3:  Concat ""| Str "@margulis2004",
            4:  Ident "cite"| Reference(3) "",
            5:  Concat ""| Str "[@steinfieldEtAl2012]",
            6:  Ident "cite"| Reference(5) "",
            7:  Concat ""| Str "\n",
                Reference(2) "", Str " the\n",
                Reference(4) "", Str " quick brown\n",
                Reference(6) "", Str " do\n\nThis is an example of sh\n\n",
            8:  Concat ""| Str "sh",
            9:  Ident "run"| Reference(8) "", Stdin "{|",
            10: Concat ""| Str "\necho yo\n",
            11: Ident "end"| Stdin "{|",
            12: Ident "references"|
            13: Concat ""| Str "\n\n\n== References\n\n", Reference(12) "", Str "\n\nstuff",
            14: Concat ""| Reference(0) "", Reference(9) "", Reference(11) "",
        );
    }

    #[test]
    #[allow(dead_code)]
    fn test_literal_sexpr() {
        make_sexpr_test!(tests::LITERAL,
            0: Concat ""| Stdin "",
            1: Concat ""| Str "\nbody 1\n",
               Literal("{|") "{{|", Str " not block ",   Literal("|}") "|}}", Str "\n",
               Literal("{$") "{{$", Str " not inline ",  Literal("$}") "$}}", Str "\n",
               Literal("{#") "{{#", Str " not comment ", Literal("#}") "#}}",
               Str "\nBody 2",
            2: Concat ""| Reference(0) "",
        );
    }

    #[test]
    #[allow(dead_code)]
    fn test_paren_sexpr() {
        make_sexpr_test!(tests::PAREN,
            0:  Concat ""| Stdin "",
            1:  Concat ""| Str "a",
            2:  Concat ""| Str "2",
            3:  Ident "cite"| Reference(1) "", Reference(2) "",

            4:  Concat ""| Str "a",
            5:  Concat ""| Str "2",
            6:  Func "cite"| Reference(4) "", Reference(5) "",
            7:  Concat ""| Reference(6) "",

            8:  Concat ""| Str "2",
            9:  Ident "cite"| Reference(8) "",
            10: Ident "cite"| Reference(9) "",

            11: Concat ""| Str "a", 12: Concat ""| Str "2",
            13: Func "cite"| Reference(11) "", Reference(12) "",
            14: Concat ""| Str "三",
            15: Func "cite"| Reference(13) "", Reference(14) "",
            16: Concat ""| Reference(15) "",

            17: Concat ""| Str "a",
            18: Concat ""| Str "2",
            19: Concat ""| Str "三",
            20: Func "cite"| Reference(18) "", Reference(19) "",
            21: Func "cite"| Reference(17) "", Reference(20) "",
            22: Concat ""| Reference(21) "",

            23: Concat ""| Str "a",
            24: Concat ""| Str "2",
            25: Concat ""| Str "三",
            26: Ident "cite"| Reference(24) "", Reference(25) "",
            27: Func "cite"| Reference(23) "", Reference(26) "",
            28: Concat ""| Reference(27) "",

            29: Concat ""| Str "a",
            30: Concat ""| Str "2",
            31: Ident "cite"| Reference(29) "", Reference(30) "",
            32: Assign "="| Ident "r", Reference(31) "",

            33: Concat ""| Str "a",
            34: Concat ""| Str "2",
            35: Func "cite"| Reference(33) "", Reference(34) "",
            36: Concat ""| Reference(35) "",
            37: Assign "="| Ident "s", Reference(36) "",

            38: Concat ""| Str "a",
            39: Concat ""| Str "2",
            40: Func "cite"| Reference(38) "", Reference(39) "",
            41: Concat ""| Str "三",
            42: Func "cite"| Reference(40) "", Reference(41) "",
            43: Concat ""| Reference(42) "",
            44: Assign "="| Ident "t", Reference(43) "",

            45: Concat ""| Str "a",
            46: Concat ""| Str "2",
            47: Concat ""| Str "三",
            48: Func "cite"| Reference(46) "", Reference(47) "",
            49: Ident "cite"| Reference(45) "", Reference(48) "",
            50: Assign "="| Ident "u", Reference(49) "",

            51: Ident "u"|
            52: Assign "="| Ident "v", Reference(51) "",
            53: Assign "="| Ident "w", Reference(52) "",
            54: Concat ""| Str "\nbody 1\n",
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
            55: Concat ""| Reference(0) "", // For the knit
        );
    }
}
