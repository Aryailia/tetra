//run: cargo test -- --nocapture

// This phase group up lexemes into s-exprs. This proceses the input lexeme
// stream into an almost topologically sorted s-expr list. ('Item::Stdin'
// needs to be resolved before it is fully topologically sorted).
// Essentially, this phase just groups arguments into s-exprs and handles
// the re-ordering of arguments due to piping.
//
// S-exprs (effectively commands) are an ordered list of lexemes. This phase
// outputs s-exprs quite verbosely. Notably, singular 'Item::Ident' are
// marked as a single s-expr, e.g.
//     '{$ cite(1, len, 3) $}'
// is parsed to:
//     (len, )
//     (cite, 1, <ref-to-len-output>, 3)
// and not:
//     (cite, 1, len, 3)
//
// This is because we do not know if 'len' is a variable or a function ident
// without the function list. This has the knock-on effect that quotes (and
// potentially other things) are also singled out as their own s-expr, e.g.
//    '{$ cite "a" $}'
// is parsed to:
//    ("a", )
//    (cite, <ref-to-a>, )
//
// Much of this over-specificity is optimised out in the next ast.rs phase.

use std::fmt::Write as _; // clippy: import without risk of name clashing

use super::{lexer::LexType, Item};
use crate::framework::{Source, Token};

pub struct SexprOutput(pub Vec<Sexpr>, pub Vec<Token<Item>>);
pub type ParseError = Token<&'static str>;

/******************************************************************************
 * Parsing
 ******************************************************************************/
// An ordered list of 'Item' that represents a function call
//
// S-expr is terminology borrowed from Lisp.
// The key differences between s-expr and full parsed functions are:
// - no identifier is marked as the function name yet
// - STDIN arguments are not resolved to which body their refer
//
// Notably, we haven't discriminated variable and function identifiers at
// this stage yet. E.g. "cite len" might `cite(len())` or `cite(len)`
#[derive(Clone, Debug)]
pub struct Sexpr {
    // for resolving to which text cell the 'Item::Stdin' refers
    pub cell_id: usize,
    // Index into {SexprOutput.1}
    pub args: (usize, usize),

    // This gets overwritten after this stage in ast.rs
    // But it is useful for reading the meaining the output of 'process()',
    // {SexprOutput}. It tells you what each {Item::Reference(x)} refers to,
    // i.e. the one that matches {x} to {Sexpr.out}.
    pub out: usize,
}

impl Sexpr {
    pub fn to_display(&self, args: &[Token<Item>], original: &str) -> String {
        let mut buffer = format!("({}): (", self.cell_id);
        for item in &args[self.args.0..self.args.1] {
            item.push_display(&mut buffer, original);
            buffer.push_str(", ");
        }
        write!(buffer, ") -> {};", self.out).unwrap();
        buffer
    }

    pub fn to_debug(&self, args: &[Token<Item>]) -> String {
        format!("({}): {:?}", self.cell_id, &args[self.args.0..self.args.1])
    }
}

impl Token<Item> {
    pub fn push_display(&self, buffer: &mut String, source: &str) {
        match self.me {
            Item::Str => write!(buffer, "{:?}", self.to_str(source)).unwrap(),
            Item::Text(s) => write!(buffer, "{:?}", s).unwrap(),
            Item::Assign => buffer.push('='),
            // This is either a variable or function identifier
            Item::Ident => buffer.push_str(self.to_str(source)),
            Item::Func => {
                buffer.push_str(self.to_str(source));
                buffer.push('(');
            }
            Item::Stdin => buffer.push('.'),
            // Temp variables for the output of concats, functions, etc.
            Item::Reference(i) => write!(buffer, "{{{}}}", i).unwrap(),

            Item::Pipe => buffer.push('|'),
            Item::PipedStdin => buffer.push_str(". | "),

            //Item::Comma => buffer.push_str("\\,"),
            Item::Comma => unreachable!(),
            Item::Paren | Item::Stmt => unreachable!(),
        }
    }

    pub fn print(&self, source: &str) {
        let mut buffer = String::new();
        self.push_display(&mut buffer, source);
        print!("{}", buffer);
    }
}

struct Fsm {
    args_cursor: usize,
    out: SexprOutput,
    buffer2: Vec<Token<Item>>, // the buffer for {to_process}
}

/******************************************************************************
 * Parsing
 ******************************************************************************/
// 'Statement' is not used strictly; it is used for explanatory purposes.
// Statements and s-exprs are both represented as a flat array of 'Item'.
// A statement is just a series of s-exprs whose end is marked by ';'
//     'a = 2'                  // single–s-expr statement (yay en-dash)
//     'a = cite(a)'            // multi–s-expr statement
//     '"hello" | cite | a ='   // multi–s-expr statement
//     'a = 2; cite(a)'         // two statements
//
// Parsing happens in three semantic stages. From an input stream of lexemes:
// 1. 'process()' maps lexemes into syntaxemes (units that mean something in
//     language syntax) and determine the end index of a statement.
// 2. 'sexprify()' breaks statements up into the individual s-exprs.
// 3. 'push_parse()' formats this slice into the s-expr data structure and does almost all the syntax checking.
// The above call stack is drilling down scheme, not in sequence.
//
// The output is structured so that:
// 1) the s-exprs are topologically sorted (with the exception as stated in
//    the introduction), i.e. ordered so that dependencies are always resolved
//    before they are needed, and
// 2) the 'Item' arguments are packed sequentially so that the output s-expr
//    data structure is just a range `(usize, usize)` that indexes into this
//    'Item' list.
////////////////////////////////////////////////////////////////////////////////

// The 'process()' function is responsible for two jobs:
// a. It exhaustively maps mutually exclusive sets of 'LexType' into a single
//    more semantic 'Item', our syntaxeme. 'Item' means just a member of an
//    s-expr (a.k.a. a list).
//
//    e.g. 'LexType::InlineStart' and 'LexType::HereDoc' both map to Item::Stmt
//
// b. It break up the stream of lexemes into statements, i.e. determine the
//    start and end index of a statement. Actually, it hands off to the next
//    logical phase whenever there is a possible s-expr, leaving unfinished
//    s-expr still in the buffer.
//
// 'process()' builds {lexemes} into a {to_process} buffer and pops off a slice
// when a s-expr boundary is found. This popping off is done by 'sexprify()'.
//

pub fn process(lexemes: &[Token<LexType>], debug_source: &str) -> Result<SexprOutput, ParseError> {
    #[derive(Debug)]
    enum Mode {
        Text,
        Code,
        Quote,
    }

    //// + 2 for the prepended initial heredoc and the knit command
    //let mut fsm = Fsm::new(lexemes.len() + 5, debug_source);
    let mut knit_sexpr = Vec::with_capacity(
        1 + lexemes
            .iter()
            .filter(|l| matches!(l.me, LexType::HereDocStart))
            .count(),
    ); // For the final document knit (concat)

    // TODO: +1 for the double push for 'LexType::Pipe' in the match below.
    //       Not 100% sure this is necessary.
    //let to_process = &mut Vec::with_capacity(lexemes.len() + 1);
    let to_process = &mut Vec::with_capacity(lexemes.len());
    let mut fsm = Fsm {
        args_cursor: 0, // cursor into {fsm.out.1}
        out: SexprOutput(
            Vec::with_capacity(lexemes.len()),
            Vec::with_capacity(lexemes.len()),
        ),
        buffer2: Vec::with_capacity(to_process.capacity()),
    };
    let mut mode = Mode::Text;
    let mut balance = Vec::new();
    // HereDoc headers get even ids, HereDoc bodies get odd ids
    // This is useful for ast.rs
    let mut cell_id = 0;

    // We act as if all documents start with an invisible heredoc at the start
    // Model after the actions of 'LexType::HereDocClose' branch
    bound_push!(fsm.out.1, Token::new(Item::Stdin, Source::Range(0, 0)));
    let out_ref = fsm.sexprify(to_process, cell_id, 0, debug_source)?;
    bound_push!(knit_sexpr, out_ref);
    cell_id += 1;

    // A statement is everything that can fit before a ';'.
    // This is to deal with 'LexType::Pipe' correctly.
    let mut stmt_cursor = 0; // Does not matter to what this is initialised

    // Potentially many 'LexType' map into a single semantic 'Item'
    // e.g. 'LexType::InlineStart' and 'LexType::HereDoc' both map to Item::Stmt
    for l in lexemes {
        //debug_print_token!(l, debug_source);
        //println!("{:?}", l);

        ////let stdin = 0;
        match (&mode, &l.me) {
            (Mode::Text, LexType::Text) => {
                //println!("{:?}", l.to_str(debug_source));
                bound_push!(to_process, l.remap(Item::Str));
            }
            (Mode::Text, LexType::BlockComment) => {} // Skip comments
            (Mode::Text, LexType::HereDocStart) => {
                mode = Mode::Code;
                // Finish up the Concat before the heredoc
                let _out_ref = fsm.sexprify(to_process, cell_id, 0, debug_source)?;
                debug_assert!(to_process.is_empty());
                //to_process.clear(); // Better worst case or fail more often?
                cell_id += 1;
                stmt_cursor = 0;
                // @TODO: check if we cannot just push (Item::Pipe, Item::Stdin)
                bound_push!(to_process, l.remap(Item::PipedStdin));
            }
            (Mode::Text, LexType::InlineStart) => {
                mode = Mode::Code;
                stmt_cursor = to_process.len();
                balance.push((Item::Stmt, to_process.len()));
            }
            (Mode::Text, LexType::Literal(s)) => bound_push!(to_process, l.remap(Item::Text(s))),

            ////////////////////////////////////////////////////////////////////
            //(Mode::Code, LexType::BlockComment) => debug_print_token!(die@l, debug_source),
            (Mode::Code, LexType::Stdin) => bound_push!(to_process, l.remap(Item::Stdin)),
            // e.g. 'cite'
            (Mode::Code, LexType::Ident) => {
                bound_push!(to_process, l.remap(Item::Ident));
            }
            // e.g. 'cite('
            (Mode::Code, LexType::IdentParen) => {
                match to_process.last().map(|t| &t.me) {
                    Some(Item::PipedStdin) => balance.push((Item::Paren, 0)),
                    Some(Item::Stdin) => todo!("sexpr.rs: Not sure if this ever possible but change this to be the same as Item::PipedStdin if it is"),
                    _ => balance.push((Item::Paren, to_process.len())),
                }
                bound_push!(
                    to_process,
                    match l.source {
                        Source::Range(a, b) =>
                            Token::new(Item::Func, Source::Range(a, b - len_utf8!('(' => 1))),
                    }
                );
            }
            // Open parenthesis necessarily means the start of a new s-expr
            // parenthesis can only be 'LexType::IdentParen' e.g. 'cite('
            // or LexType::ParenStart '(cite ...'
            (Mode::Code, LexType::ParenStart) => {
                balance.push((Item::Paren, to_process.len()));
            }
            (Mode::Code, LexType::ParenClose) => {
                // check for paren balance
                let start = match balance.pop() {
                    Some((Item::Paren, x)) => x,
                    _ => return Err(l.remap("Unbalanced parenthesis")),
                };
                let out_ref = fsm.sexprify(to_process, cell_id, start, debug_source)?;
                bound_push!(to_process, out_ref);
            }

            (Mode::Code, LexType::Pipe) => {
                let out_ref = fsm.sexprify(to_process, cell_id, stmt_cursor, debug_source)?;
                // Only place that we double push onto {to_process}, but
                // we set len to 0 with the `fsm.sexprify()`.
                // The capacity + 1 pre-for-loop accounts for this extra push
                bound_push!(to_process, l.remap(Item::Pipe));
                bound_push!(to_process, out_ref);
            }
            // a.k.a. the end of a block code cell '|}'
            (Mode::Code, LexType::HereDocClose) => {
                mode = Mode::Text;
                let out_ref = fsm.sexprify(to_process, cell_id, 0, debug_source)?;
                bound_push!(knit_sexpr, out_ref);
                //bound_push!(to_process, l.remap(Item::Concat));
                cell_id += 1;
            }
            // a.k.a. the end of an inline code cell '$}'
            (Mode::Code, LexType::InlineClose) => {
                mode = Mode::Text;
                let start = match balance.pop() {
                    Some((Item::Stmt, x)) => x,
                    x => unreachable!("{:?}", x),
                };
                let out_ref = fsm.sexprify(to_process, cell_id, start, debug_source)?;
                bound_push!(to_process, out_ref);
            }

            (Mode::Code, LexType::QuoteStart) => {
                mode = Mode::Quote;
                balance.push((Item::Str, to_process.len()));
            }
            // TODO: Rename to CmdSeparator StmtSeparator
            (Mode::Code, LexType::CmdSeparator) => {
                // "display ''; cite" means we ignore the output of the first command
                let _out_ref = fsm.sexprify(to_process, cell_id, stmt_cursor, debug_source)?;
                stmt_cursor = to_process.len();
            }

            (Mode::Code, LexType::Assign) => {
                bound_push!(to_process, l.remap(Item::Assign));
            }
            (Mode::Code, LexType::ArgSeparator) => {
                bound_push!(to_process, l.remap(Item::Comma));
            }
            //(Mode::Code, _) => return Err(Token::new("Sexpr.rs: Unhandled token", source)),
            ////(Mode::Code, _) => debug_print_token!(die@l, debug_source),

            ////////////////////////////////////////////////////////////////////
            // @TODO: What should happen with quotes in succession without
            //        whitespace separator e.g. `cite "jane"'doe'`
            (Mode::Quote, LexType::Quoted) => {
                bound_push!(to_process, l.remap(Item::Str));
            }
            (Mode::Quote, LexType::QuoteEscaped(s)) => {
                bound_push!(to_process, l.remap(Item::Text(s)));
            }
            (Mode::Quote, LexType::QuoteClose) => {
                mode = Mode::Code;
                let start = match balance.pop() {
                    Some((Item::Str, x)) => x,
                    x => unreachable!("{:?}", x),
                };
                let out_ref = fsm.sexprify(to_process, cell_id, start, debug_source)?;
                bound_push!(to_process, out_ref);
            }
            //(Mode::Quote, _) => debug_print_token!(die@l, debug_source),
            _ => {
                println!("\n\nsexpr.rs: {:?}", l);
                return Err(l.remap("sexpr.rs: Unhandled case"));
            }
        }

        // In accordance to the notes in the definition of 'Item'
        debug_assert!(!to_process
            .iter()
            .any(|t| matches!(t.me, Item::Paren | Item::Stmt)));
        debug_assert!(!fsm
            .out
            .1
            .iter()
            .any(|t| matches!(t.me, Item::Comma | Item::Paren | Item::Stmt)));
    }
    // End the final heredoc body
    let _out_ref = fsm.sexprify(to_process, cell_id, 0, debug_source)?;

    // Add the {knit_expr}
    fsm.sexprify(&mut knit_sexpr, cell_id + 1, 0, debug_source)?;

    //for p in &fsm.output {
    //    println!(" sexpr  {}", p.to_display(&fsm.args, debug_source));
    //}
    //for p in buffer {
    //    print!(" remaining  ");
    //    debug_print_token!(p, debug_source);
    //}
    Ok(fsm.out)
    //Err(Token::new("Finished parsing", Source::Range(0, 0)))
}

// Breaks off as many valid s-exprs as possible from {to_process}, pushing
// them into {out.1}.
// Called at the lexemes: ')' ';' '|' '$}' '|}' ','
//
// As input, we know for certain that {to_process} is the end of a valid s-expr.
// {start} marks the beginning of the first valid s-expr: there could be many
// s-exprs in {to_process}. {start} is determined by 'process()'.
//
// {cell_id} is the mechanism by which we keep track of which heredoc bodies
// (a.k.a. text cells) match with which heredoc headers (a.k.a code cells).
//
// This is determines the start index of an s-expr.
//
// 'process()' determines that:
//     'foo(a = bar("b"), ...'
//  should hand off to 'exprify()' at:
//     'foo(a = bar("b")'
//
// 'sexprify()' then breaks that up into:
//     1: "b"
//     2: (foo, {1}, )
//     3: (=, a, {2}, )
// leaving "foo(" in {to_process} for when 'foo()' is provided all its
// arguments.
//
// Syntax checking of these s-exprs is handed off to 'parse_push()'.
impl Fsm {
    fn form_sexpr_and_update_cursor(&mut self, cell_id: usize) -> (Sexpr, Token<Item>) {
        let output_id = self.out.0.len();
        let sexpr = Sexpr {
            cell_id,
            args: (self.args_cursor, self.out.1.len()),
            out: output_id,
        };
        self.args_cursor = self.out.1.len(); // Set before pushing infix operator

        // TODO: make this the span of the 'Sexpr'
        let source = Source::Range(0, 0);
        let out_ref = Token::new(Item::Reference(output_id), source);
        (sexpr, out_ref)
    }

    fn sexprify(
        &mut self,
        to_process: &mut Vec<Token<Item>>,
        cell_id: usize,
        start: usize,
        _debug_source: &str,
    ) -> Result<Token<Item>, ParseError> {
        // First sexpr does not have an infix operator
        Ok(loop {
            // Index after the infix operator
            let post_infix = to_process[start..]
                .iter()
                // Match any of the infix operators
                .rposition(|t| matches!(t.me, Item::Assign))
                .map(|i| i + 1)
                .unwrap_or(0);

            // '{$ a = $}' and '{| a = $}' will lead to the following being true
            if to_process[start + post_infix..].is_empty() && post_infix != 0 {
                self.out.1.push(to_process.pop().unwrap());
                continue;
            }

            self.parse_push(to_process.drain(start + post_infix..), cell_id)?;
            // Set {self.args_cursor} before pushing infix operator
            let (sexpr, out_ref) = self.form_sexpr_and_update_cursor(cell_id);

            //// This is how we debug stuff
            //println!("{}", sexpr.to_display(args, _debug_source));
            //sexpr.print_debug(&self.out.1);

            self.out.0.push(sexpr);

            // Infix operator present, so multiple sexpr
            if post_infix != 0 {
                // Push the infix operator as a prefix operator
                self.out.1.push(to_process.pop().unwrap());
                to_process.push(out_ref);

            // Infix operator absent, so just one complete sexpr
            } else {
                // Delegate {out_ref} push to {to_process} to outside
                break out_ref;
            }
        })
    }

    // Syntax check an sexpr, and then pushes it onto {args}.
    // Also moves piped args to the last argument.
    fn parse_push(
        &mut self,
        to_process: std::vec::Drain<Token<Item>>,
        cell_id: usize,
    ) -> Result<(), ParseError> {
        // Imagine we are building a function. Broadly, the cases are:
        // 1. 'first(arg1, arg2, arg3...)'
        // 2. '| "hello" first(arg1, arg2, arg3...)
        //    or
        //    '.|  first(arg1, arg2, arg3...)'
        // 3. concat is the default when function ident is not an Item::Ident
        enum M {
            // Handling the first argument
            First,    // As in the (potential) function ident
            PipedArg, // First arg was a Item::Pipe, so the piped arg

            PipelessFirst, // Now that piped is handled, definintely function ident
            Concat,        // If not an 'Item::Ident' or 'Item::Func', then default here

            // Handling the other arguments for a fucntion call
            ExpectArg,
            ExpectComma,
        }
        let mut piped_arg: Option<Token<Item>> = None;
        let mut state = M::First;
        let mut prev = &Item::Str;

        for item in to_process {
            match (&state, &item.me) {
                (_, Item::Paren | Item::Stmt) => unreachable!(),

                // Might start with pipes
                (M::First, Item::Pipe) => {
                    state = M::PipedArg;
                    continue // Do not push the Item::Pipe
                }
                (M::First, Item::PipedStdin) => {
                    piped_arg = Some(item.remap(Item::Stdin));
                    state = M::PipelessFirst;
                    continue // Do not push because {item} is taken by {piped_arg}
                }
                // Else, it is a function call if {item} is ident
                (M::First, Item::Ident | Item::Func) => state = M::ExpectArg,


                // or just the first arg of a concat otherwise
                (M::First, Item::Comma) => return Err(item.remap("Unexpected comma. Interpreting the previous Ident as a function call. Should this comma be a open parenthesis?")),
                (M::First, _) =>  state = M::Concat,

                (_,  Item::Pipe) => return Err(item.remap("You can not have double pipes")),
                (_,  Item::PipedStdin) => unreachable!(),

                // Second argument if first was a pipe
                (M::PipedArg, _) => {
                    piped_arg = Some(item);
                    state = M::PipelessFirst;
                    continue // Do not push because {item} is taken by {piped_arg}
                }

                // The first without pipes or second/third argument after pipes arg
                // Like M::First (either the function ident or first arg of concat),
                // except there should be no pipes
                (M::PipelessFirst, Item::Ident | Item::Func) => state = M::ExpectArg,
                (M::PipelessFirst, _) => state = M::Concat,

                // Above should catch all the non-argument entries
                (M::Concat, Item::Comma) => return Err(item.remap("Unexpected comma. There is no function call for this list of arguments.")),
                (M::Concat, _) => {} // Concat accepts all arguments

                (M::ExpectArg, Item::Comma) => return Err(item.remap("No value provided")),
                (M::ExpectArg, Item::Ident) => {
                    // Single argument idents must be pushed as their own
                    // s-expr since we cannot determine if they are variables
                    // or function calls
                    bound_push!(self.out.1, item);

                    let (sexpr, out_ref) = self.form_sexpr_and_update_cursor(cell_id);
                    self.buffer2.push(out_ref);
                    bound_push!(self.out.0, sexpr);
                    prev = &Item::Reference(usize::MAX);

                    state = M::ExpectComma;
                    continue
                }
                (M::ExpectArg, _) => state = M::ExpectComma,

                (M::ExpectComma, Item::Comma) => {
                    state = M::ExpectArg;
                    continue // Do not push
                }
                (M::ExpectComma, _) if matches!(prev, Item::Ident | Item::Func) => {
                    return Err(item.remap("Expected comma. If this is part of a function call, you need a paren to disambiguate this."));
                }
                (M::ExpectComma, _) => return Err(item.remap("Expect a comma before here.")),

            }
            bound_push!(self.buffer2, item);
            prev = &self.buffer2[self.buffer2.len() - 1].me;
        }
        if let Some(a) = piped_arg {
            bound_push!(self.buffer2, a);
        }

        // In accordance to the notes in the definition of 'Item'
        debug_assert!(!self
            .buffer2
            .iter()
            .any(|t| matches!(t.me, Item::Comma | Item::Paren | Item::Stmt)));
        self.out.1.append(&mut self.buffer2);
        Ok(())
    }
}

/******************************************************************************
 * Helpers
 ******************************************************************************/
// TODO: put the spans for 'Item::Reference()'
//fn source_span((start, close): (usize, usize), args: &[Token<Arg>]) -> Source {
//    let parameters = &args[start..close];
//
//    // All {args[].source} should only be Source::Range
//    #[cfg(debug_assertions)]
//    {
//        for p in parameters {
//            debug_assert!(matches!(p.source, Source::Range(_, _)));
//        }
//    }
//
//    // Default to (0, 0) when it is an empty statement
//    let source1 = match parameters.first().map(|p| &p.source) {
//        Some(Source::Range(a, _)) => *a,
//        //Some(_) => unreachable!(),
//        _ => 0,
//    };
//    let source2 = match parameters.last().map(|p| &p.source) {
//        Some(Source::Range(_, b)) => *b,
//        //Some(_) => unreachable!(),
//        _ => 0,
//    };
//    Source::Range(source1, source2)
//}
