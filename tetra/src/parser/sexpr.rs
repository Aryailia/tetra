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

use super::{lexer::LexType, Item, Label, Param};
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
#[derive(Debug)]
pub struct Sexpr {
    // for resolving to which text cell the 'Item::Stdin' refers
    pub cell_id: usize,
    pub head: Token<Label>,
    // Index into {SexprOutput.1}
    pub args: (usize, usize),

    // This gets overwritten after this stage in ast.rs
    // But it is useful for reading the meaining the output of 'process()',
    // {SexprOutput}. It tells you what each {Item::Reference(x)} refers to,
    // i.e. the one that matches {x} to {Sexpr.out}.
    pub output_id: usize,
}

impl Sexpr {
    pub fn to_display(&self, args: &[Token<Item>], original: &str) -> String {
        let mut buffer = format!("({}): (", self.cell_id);
        self.head.push_display(&mut buffer, original);
        buffer.push_str(" <> ");
        for item in &args[self.args.0..self.args.1] {
            item.push_display(&mut buffer, original);
            buffer.push_str(", ");
        }
        write!(buffer, ") -> {};", self.output_id).unwrap();
        buffer
    }
    pub fn to_display2(&self, args: &[Token<Param>], original: &str) -> String {
        let mut buffer = format!("({}): (", self.cell_id);
        self.head.push_display(&mut buffer, original);
        buffer.push_str(" <> ");
        for item in &args[self.args.0..self.args.1] {
            item.push_display(&mut buffer, original);
            buffer.push_str(", ");
        }
        write!(buffer, ") -> {};", self.output_id).unwrap();
        buffer
    }

    pub fn to_debug(&self, args: &[Token<Item>]) -> String {
        format!("({}): {:?}", self.cell_id, &args[self.args.0..self.args.1])
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
            // @TODO: Justify this + 2, small tests in "syntax.rs" were
            //        running into this bound with 'push_parse()'
            Vec::with_capacity(lexemes.len() + 2),
            Vec::with_capacity(lexemes.len() + 2),
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
            (Mode::Text, LexType::Literal(s)) => bound_push!(to_process, l.remap(Item::Literal(s))),

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
            (Mode::Quote, LexType::Text) => {
                bound_push!(to_process, l.remap(Item::Str));
            }
            (Mode::Quote, LexType::QuoteLiteral(s)) => {
                bound_push!(to_process, l.remap(Item::Literal(s)));
            }
            (Mode::Quote, LexType::QuoteClose) => {
                mode = Mode::Code;
                let start = match balance.pop() {
                    Some((Item::Str, i)) => i,
                    x => unreachable!("{:?}", x),
                };
                // For the case of the empty quote '""', instead of pushing
                // an empty concat `(concat,)`, push a 'Item::Literal()'
                if start == to_process.len() {
                    match l.source {
                        Source::Range(a, b) => to_process.push(Token::new(
                            Item::Literal(""), Source::Range(a - len_utf8!('"' => 1), b)
                        )),
                    }
                    // This is because the optimiser step in "ast.rs" only
                    // copies the literals of 'Label::Concat' with one arg
                }
                let out_ref = fsm.sexprify(to_process, cell_id, start, debug_source)?;
                bound_push!(to_process, out_ref);
            }
            //(Mode::Quote, _) => debug_print_token!(die@l, debug_source),
            _ => {
                println!("\n\nsexpr.rs: {:?}", l);
                return Err(l.remap("sexpr.rs: Unhandled case"));
            }
        }
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
    fn sexprify(
        &mut self,
        to_process: &mut Vec<Token<Item>>,
        cell_id: usize,
        start: usize,
        _debug_source: &str,
    ) -> Result<Token<Item>, ParseError> {
        // First sexpr does not have an infix operator
        let mut close = to_process.len();
        Ok(loop {
            // Index after the infix operator
            let post_infix = to_process[start..close]
                .iter()
                // Match any of the infix operators
                .rposition(|t| matches!(t.me, Item::Assign))
                .map(|i| i + 1)
                .unwrap_or(0);

            // '{$ a = $}' and '{| a = |}' will lead to the following being true
            // In other words the slice we are processing is the empty list.
            // Without this it pushes an s-expr to {self.out.0} which causes
            // the 'Item::Reference()' to be incorrect, e.g.
            //     '{| a = |}'
            // parses to:
            //     1: (Concat | "", )
            //     2: (= | a, {2}, ., )
            //
            // we want:
            //     :  (= | a, ., )
            if to_process[start + post_infix..].is_empty() && post_infix != 0 {
                close -= 1; // Skip over the infix operator
                continue;
            }

            // Set {self.args_cursor} before pushing infix operator
            let (sexpr, out_ref) =
                self.parse_push(to_process.drain(start + post_infix..), cell_id)?;

            //// This is how we debug stuff
            //println!("{}", sexpr.to_display(&self.out.1, _debug_source));
            //sexpr.print_debug(&self.out.1);

            self.out.0.push(sexpr);

            // Infix operator present, so multiple sexpr
            if post_infix != 0 {
                //// Push the infix operator as a prefix operator
                //self.out.1.push(to_process.pop().unwrap());
                close = to_process.len() - 1; // Skip over the infix operator
                to_process.push(out_ref);

            // Infix operator absent, so just one complete sexpr
            } else {
                // Delegate {out_ref} push to {to_process} to outside
                break out_ref;
            }
        })
    }

    // This has four main jobs:
    // 1. Syntax checking
    // 2. Re-order piped-in arguments to the last argument
    // 3. Break off single 'Item::Ident' arguments, e.g.
    //        concat(a, cite("s"))
    //    should parse to
    //        (a, )
    //        concat(<ref-to-a>, <ref-to-cite>)
    //    This is so that it can be interpreted as a variable lookup or a
    //    a function call in the run phase.
    //
    // 4. Push the final s-exprs into the output array
    // Also moves piped args to the last argument.
    fn parse_push(
        &mut self,
        to_process: std::vec::Drain<Token<Item>>,
        cell_id: usize,
    ) -> Result<(Sexpr, Token<Item>), ParseError> {
        // Imagine we are building a function. Broadly, the cases are:
        // 1. 'first(arg1, arg2, arg3...)'
        // 2. '| "hello" first(arg1, arg2, arg3...)
        //    or
        //    '.|  first(arg1, arg2, arg3...)'
        // 3. concat is the default when function ident is not an Item::Ident
        #[derive(Debug)]
        enum M {
            // Handling the first argument and maybe pipe logic
            First,         // As in the (potential) function ident
            PipelessFirst, // Now that piped is handled, definintely function ident

            // Different types of arguments
            Concat, // If not an 'Item::Ident' or 'Item::Func', then default here
            Assign,

            // Handling the other arguments for a fucntion call
            ExpectArg,
            ExpectComma,
        }
        let mut piped_arg: Option<Token<Item>> = None;
        let mut state = M::First;
        let mut head = None;

        let mut iter = to_process.peekable();
        while let Some(item) = iter.next() {
            let peek = iter.peek().map(|t| &t.me);
            match (&state, &item.me) {
                (_, Item::Paren | Item::Stmt) => unreachable!(),

                ////////////////////////////////////////////////////////////////
                // Determine what kind of s-expr it is: Assign, Concat, Function
                // Might start with pipes
                (M::First, Item::Pipe) => {
                    if let Some(arg) = iter.next() {

                        match &arg.me {
                            //Item::Colon |
                                Item::Comma => {
                                return Err(arg.remap("Expecting an argument before the pipe."))
                            }
                            _ => piped_arg = Some(arg),
                        }
                    }
                    state = M::PipelessFirst;

                }
                (M::First, Item::PipedStdin) => {
                    piped_arg = Some(item.remap(Item::Stdin));
                    state = M::PipelessFirst;
                }

                // 'process()' ensures we only ever have pipe per statement
                (_, Item::PipedStdin | Item::Pipe) => unreachable!(),

                ////////////////////////////////////////////////////////////////
                // First but without pipes
                //(M::First | M::PipelessFirst, Item::Colon) => {
                //    return Err(item.remap("Unexpected comma. Interpreting the previous Ident as a function call. Should this comma be a open parenthesis?"))
                //}
                (M::First | M::PipelessFirst, Item::Comma) => {
                    return Err(item.remap("Unexpected comma. Interpreting the previous Ident as a function call. Should this comma be a open parenthesis?"))
                }
                (M::First | M::PipelessFirst, Item::Ident | Item::Func) if matches!(peek, Some(Item::Assign)) => {
                    state = M::Assign;
                    debug_assert!(head.is_none());
                    head = iter.next().map(|item| item.remap(Label::Assign));
                    bound_push!(self.out.1, item);

                }
                (M::First | M::PipelessFirst, Item::Ident) => {
                    state = M::ExpectArg;
                    debug_assert!(head.is_none());
                    head = Some(item.remap(Label::Ident));
                }
                (M::First | M::PipelessFirst, Item::Func) => {
                    state = M::ExpectArg;
                    debug_assert!(head.is_none());
                    head = Some(item.remap(Label::Func));
                }
                (M::First | M::PipelessFirst, _) => {
                    state = M::Concat;
                    bound_push!(self.out.1, item);
                }

                // 'sexprify()' `.rposition()` ensures we only have one
                // 'Item::Assign' per 'push_parse()' call
                (_, Item::Assign) => return Err(item.remap("Unexpected assign")),


                ////////////////////////////////////////////////////////////////
                // S-expr type determined

                // Above should catch all the non-argument entries
                (M::Concat | M::Assign, Item::Comma) => return Err(item.remap("Unexpected comma. There is no function call for this list of arguments.")),
                (M::Concat, _) => bound_push!(self.buffer2, item),
                (M::Assign, _) => bound_push!(self.buffer2, item),





                ////////////////////////////////////////////////////////////////
                // Function

                (M::ExpectArg, Item::Comma) => return Err(item.remap("No value provided")),
                (M::ExpectArg, _) if matches!(peek, Some(Item::Ident | Item::Func)) => {
                    return Err(item.remap("Expected comma. If this is part of a function call, you need a paren to disambiguate this."));
                }
                (M::ExpectArg, Item::Ident) => {
                    // Single argument idents must be pushed as their own
                    // s-expr since we cannot determine if they are variables
                    // or function calls
                    //bound_push!(self.out.1, item);

                    let source = item.source.clone();
                    let label = item.remap(Label::Ident);
                    let (sexpr, out_item) = self.form_sexpr_and_update_cursor(label, cell_id);
                    self.buffer2.push(Token::new(out_item, source));
                    bound_push!(self.out.0, sexpr);

                    state = M::ExpectComma;
                    continue
                }
                (M::ExpectArg, _) => {
                    state = M::ExpectComma;
                    bound_push!(self.buffer2, item);
                }

                (M::ExpectComma, Item::Comma) => {
                    state = M::ExpectArg;
                    continue // Do not push
                }
                (M::ExpectComma, _) => return Err(item.remap("Expect a comma before here.")),

            }
        }
        if let Some(a) = piped_arg {
            bound_push!(self.buffer2, a);
        }

        // In accordance to the notes in the definition of 'Item'
        debug_assert!(!self
            .buffer2
            .iter()
            .any(|t| matches!(t.me, Item::Concat | Item::Comma | Item::Paren | Item::Stmt)));
        self.out.1.append(&mut self.buffer2);

        // @TODO: make this the span of the 'Sexpr'
        let source = Source::Range(0, 0);
        let default = Token::new(Label::Concat, source.clone());
        let (sexpr, out_item) = self.form_sexpr_and_update_cursor(head.unwrap_or(default), cell_id);
        Ok((sexpr, Token::new(out_item, source)))
    }

    fn form_sexpr_and_update_cursor(
        &mut self,
        head: Token<Label>,
        cell_id: usize,
    ) -> (Sexpr, Item) {
        let output_id = self.out.0.len();
        let sexpr = Sexpr {
            cell_id,
            head,
            args: (self.args_cursor, self.out.1.len()),
            output_id,
        };
        self.args_cursor = self.out.1.len(); // Set before pushing infix operator

        (sexpr, Item::Reference(output_id))
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
