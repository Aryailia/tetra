//run: cargo test -- --nocapture

// This decides the grouping of lexemes into commands as well as breaking up
// multi-statement commands into single commands.
// e.g. `a = b = c` -> `b = c` and `a = <output>`
//
// We call these commands s-exprs because we have not parsed out the
// final order of the arguments quite yet.

use crate::framework::{Source, Token};
use crate::lexer::LexType;

pub type ParseOutput = (Vec<Sexpr>, Vec<Token<Arg>>);
pub type ParseError = Token<&'static str>;

//#[derive(Debug, Eq, PartialEq)]
//pub enum Node {
//    Const(i64),
//    Literal(usize),
//    Function { args: Vec<usize> },
//    List(Vec<Node>),
//    //Add {
//    //    lhs: AstNodeId,
//    //    rhs: AstNodeId,
//    //},
//    //Sub {
//    //    lhs: AstNodeId,
//    //    rhs: AstNodeId,
//    //},
//    //Mul {
//    //    lhs: AstNodeId,
//    //    rhs: AstNodeId,
//    //},
//    //Div {
//    //    lhs: AstNodeId,
//    //    rhs: AstNodeId,
//    //},
//}

macro_rules! debug_print_token {
    ($token:expr, $source_text:expr) => {
        println!(
            "{:<50} {:?}",
            format!("{:?}", $token),
            $token.to_str($source_text)
        )
    };

    (die@ $token:expr, $source_text:expr) => {
        unreachable!(
            "\n===\n{:<50} {:?}\n===\n",
            format!("{:?}", $token),
            $token.to_str($source_text)
        )
    };
}

// Processes a lexeme list into an almost topologically sorted s-expr list.
// (Everything but STDIN is topologically sorted)
// Essentially it just groups arguments into individual commands
//
// S-expr is terminology borrowed from Lisp.
// The key differences between s-expr and full parsed functions are:
// - parameter piping reordering is not yet completed
//   - e.g. (s-expr order) "5 | cite 3" -> (function order) "cite 3, 5"
// - no identifier is marked as the function name yet
// - STDIN arguments are not resolved to which body their refer
//
// Additionally, we haven't discriminated variable and function identifiers at
// this stage yet. E.g. "cite len, a" might all be functions.

pub fn process(lexemes: &[Token<LexType>], debug_source: &str) -> Result<ParseOutput, ParseError> {
    //let mut arena = Arena::<Node>::new();

    //// Create the AST for `a * (b + 3)`.
    ////let three_id = ast_nodes.alloc(AstNode::Const(3));
    ////let b = ast_nodes.alloc(AstNode::Var("b".into()));
    ////let b_plus_three = ast_nodes.alloc(AstNode::Add {
    ////    lhs: b,
    ////    rhs: three_id,
    ////});
    ////let a = ast_nodes.alloc(AstNode::Var("a".into()));
    ////let a_times_b_plus_three = ast_nodes.alloc(AstNode::Mul {
    ////    lhs: a,
    ////    rhs: b_plus_three,
    ////});

    //// Can use indexing to access allocated nodes.
    ////assert_eq!(ast_nodes[three_id], AstNode::Const(3));

    let mut fsm = Fsm::new(lexemes.len());
    //for l in lexemes {
    //    debug_print_token!(l, debug_source);
    //}

    let mut knit_sexpr = Vec::new(); // For the final document concat
    let mut cell_id = 0; // HereDoc headers and bodies get different id's

    // We act as if all documents start with an invisible heredoc at the start
    // Model after the actions of 'LexType::HereDocClose' branch
    fsm.args
        .push(Token::new(Arg::PipedStdin, Source::Range(0, 0)));
    let output_token = fsm.drain_push_sexpr(cell_id, 0);
    knit_sexpr.push(output_token);
    cell_id += 1;

    for l in lexemes {
        //debug_print_token!(l, debug_source);
        ////let stdin = 0;
        let source = l.source.clone();

        match (&fsm.mode, &l.me) {
            (Mode::Text, LexType::Text) => {
                //println!("{:?}", l.to_str(debug_source));
                fsm.buffer.push(Token::new(SexprType::Str, source));
            }
            (Mode::Text, LexType::BlockComment) => {} // Skip comments
            (Mode::Text, LexType::HereDocStart) => {
                fsm.mode = Mode::Code;
                let _output_token = fsm.drain_push_sexpr(cell_id, 0);
                cell_id += 1;

                // @TODO: Double check that we have to push SexprType::NewFunction
                // We push SexprType::NewFunction for multiple commands in the
                // code body, e.g. "{| cite ''; print() |}"

                fsm.buffer.clear(); // Not algorithmically necessary, but clears memory
                fsm.buffer
                    .push(Token::new(SexprType::NewFunction, source.clone()));
                fsm.buffer.push(Token::new(SexprType::PipedStdin, source));
            }
            (Mode::Text, LexType::InlineStart) => {
                fsm.mode = Mode::Code;
                fsm.buffer.push(Token::new(SexprType::NewFunction, source));
            }
            (Mode::Text, _) => debug_print_token!(die@l, debug_source),

            ////////////////////////////////////////////////////////////////////
            (Mode::Code, LexType::BlockComment) => debug_print_token!(die@l, debug_source),
            (Mode::Code, LexType::Stdin) => {
                fsm.buffer.push(Token::new(SexprType::Stdin, source));
                //args.push(Token::new());
            }
            (Mode::Code, LexType::Ident) => {
                fsm.buffer.push(Token::new(SexprType::Ident, source));
            }
            (Mode::Code, LexType::IdentParen) => {
                fsm.buffer.push(Token::new(SexprType::IdentFunc, source));
            }
            (Mode::Code, LexType::Pipe) => {
                let output_token = fsm.drain_push_sexpr(cell_id, 0);
                fsm.buffer.push(output_token);
                fsm.buffer.push(Token::new(SexprType::Pipe, source));
            }
            (Mode::Code, LexType::HereDocClose) => {
                fsm.mode = Mode::Text;
                let output_token = fsm.drain_push_sexpr(cell_id, 0);
                knit_sexpr.push(output_token);
                cell_id += 1;
                //fsm.buffer.push(output_token);
            }
            (Mode::Code, LexType::InlineClose) => {
                fsm.mode = Mode::Text;
                let output_token = fsm.drain_push_sexpr(cell_id, 0);
                fsm.buffer.push(output_token);
            }

            (Mode::Code, LexType::QuoteStart) => {
                fsm.mode = Mode::Quote;
                fsm.buffer.push(Token::new(SexprType::NewFunction, source));
            }
            (Mode::Code, LexType::CmdSeparator) => {
                // "display ''; cite" means we ignore the output of the first command
                let _output_token = fsm.drain_push_sexpr(cell_id, 0);
                fsm.buffer.push(Token::new(SexprType::NewFunction, source));
            }
            // After lexing, open parenthesis mean a function call
            //(Mode::Code, LexType::ParenStart) => {
            //    //let func_ident = fsm.buffer.pop().unwrap();
            //    //debug_assert!(matches!(func_ident.me, SexprType::Ident));
            //    ////println!("{:?}", fsm.buffer);
            //    //fsm.buffer.push(Token::new(SexprType::NewFunction, source));
            //    //fsm.buffer.push(func_ident);
            //}
            (Mode::Code, LexType::ParenClose) => {
                // if(cite a)
                // if cite(a)
                //let output_token = fsm.drain_push_sexpr(cell_id, 0);
                //fsm.buffer.push(output_token);
                let i = fsm
                    .buffer
                    .iter()
                    .rposition(|t| matches!(t.me, SexprType::IdentFunc))
                    .unwrap();
                // Push the interior of the parenthesis
                // e.g. `cite(ref 1)` -> push `ref 1`
                let output_token = fsm.drain_push_sexpr(cell_id, i + 1);
                fsm.buffer.push(output_token);

                // If there was a pipe then process normally
                if matches!(
                    fsm.buffer[i - 1].me,
                    SexprType::Pipe | SexprType::PipedStdin
                ) {
                    let output_token = fsm.drain_push_sexpr(cell_id, 0);
                    fsm.buffer.push(output_token);
                // Otherwise we are nested within another function call and
                // only process the function itself
                // e.g. `cite(ref 1)` -> push `cite(<output>)` after the first push
                } else {
                    let output_token = fsm.drain_push_sexpr(cell_id, i);
                    fsm.buffer.push(output_token);
                    //fsm.buffer.push(Token::new(SexprType::NewFunction, source));
                }
            }
            (Mode::Code, LexType::Assign) => {
                fsm.buffer.push(Token::new(SexprType::Assign, source));
            }
            (Mode::Code, _) => return Err(Token::new("Sexpr.rs: Unhandled token", source)),
            //(Mode::Code, _) => debug_print_token!(die@l, debug_source),

            ////////////////////////////////////////////////////////////////////
            // @TODO: What should happen with quotes in succession without
            //        whitespace separator e.g. `cite "jane"'doe'`
            (Mode::Quote, LexType::Quoted) => {
                fsm.buffer.push(Token::new(SexprType::Str, source));
            }
            (Mode::Quote, LexType::QuoteEscaped(c)) => {
                fsm.buffer.push(Token::new(SexprType::Char(*c), source));
            }
            (Mode::Quote, LexType::QuoteClose) => {
                fsm.mode = Mode::Code;
                let output_token = fsm.drain_push_sexpr(cell_id, 0);
                fsm.buffer.push(output_token);
            }
            (Mode::Quote, _) => debug_print_token!(die@l, debug_source),
        }
    }

    // Push the final heredoc body as a concat-display command
    // Model this after LexType::HereDocStart branch of match
    let _output_token = fsm.drain_push_sexpr(cell_id, 0);
    // Do not push this {_output_token} into the buffer
    fsm.buffer.extend(knit_sexpr);
    fsm.drain_push_sexpr(cell_id + 1, 0);

    //for p in fsm.buffer {
    //    print!(" remaining  ");
    //    debug_print_token!(p, debug_source);
    //}
    //Err(Token::new("Finished parsing", Source::Range(0, 0)))
    Ok((fsm.output, fsm.args))
}

struct Cursor(usize);
impl Cursor {
    #[inline]
    fn move_to(&mut self, target: usize) -> (usize, usize) {
        let start = self.0;
        self.0 = target;
        (start, target)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arg {
    //Literal(usize, usize), // Range indexing into vec[] for {Literal}'s
    Str,
    Char(char),
    Ident,            // Either a variable of function Ident
    IdentFunc,        // Just a function Ident
    Reference(usize), // Index into {args} pointing to an 'Arg::Output'
    Stdin,
    Assign,
    // @TODO: Check if this has to be different from Arg::Stdin
    //        I expect this to catch ". cite" expressions
    PipedStdin,
    Pipe,
}

#[derive(Debug)]
enum SexprType {
    Str,        // Index into {data} array
    Char(char), // Index into {data} array
    Ident,
    IdentFunc,
    Stdin,
    Assign,
    Pipe,
    PipedStdin,
    Reference(usize), // Id that should match {Sexpr.out}
    NewFunction,
}

#[derive(Debug)]
enum Mode {
    Text,
    Code,
    Quote,
}

struct Fsm {
    mode: Mode,
    output: Vec<Sexpr>,

    args: Vec<Token<Arg>>,
    args_cursor: Cursor,
    args_stdin_index: usize,

    stdin_range: (usize, usize),
    buffer: Vec<Token<SexprType>>,
    //stack: Vec<usize>, // @TODO: for paraenthesis and bracket balancing
}

impl Fsm {
    fn new(capacity: usize) -> Self {
        // Parsing into sexpr will never produce more than the number of lexemes
        // as we never push
        Self {
            mode: Mode::Text,
            output: Vec::with_capacity(capacity),

            args: Vec::with_capacity(capacity),
            args_cursor: Cursor(0),
            args_stdin_index: 0,

            stdin_range: (0, 0),
            buffer: Vec::with_capacity(capacity),
            //stack: Vec::new(),
        }
    }

    // 'drain_push_sexpr' is called the lexemes have signaled an s-expr has
    // been completed in the match branches. {cell_id} assigns what heredoc
    // cell this command belongs to.
    //
    // On {start}:
    // Normally, we drain starting from the last 'SexprType::NewFunction'
    // in {self.buffer}. {start} is there to limit this range even more for
    // when there might be multiple commands in a single line, e.g.
    // `cite(cite(a))` -> `cite(a)` `cite(<output>)`
    //
    // However, this does not cover the infix case. For parenthesis, we only
    // emit one s-expr; for infix, we emit at least two.
    fn drain_push_sexpr(&mut self, cell_id: usize, start: usize) -> Token<SexprType> {
        fn parse_push_s_args(args: &mut Vec<Token<Arg>>, parameters: Drain<Token<SexprType>>) {
            for p in parameters {
                //debug_print_token!(p, debug_source);
                match p.me {
                    SexprType::Str => args.push(p.remap(Arg::Str)),
                    SexprType::Char(c) => args.push(p.remap(Arg::Char(c))),
                    SexprType::Assign => args.push(p.remap(Arg::Assign)),
                    SexprType::Ident => args.push(p.remap(Arg::Ident)),
                    // @TODO: Should we resolve unknowns (variable or function
                    //        idents) to function idents?
                    SexprType::IdentFunc => args.push(Token::new(
                        Arg::IdentFunc,
                        // Remove the paren at the end of the source
                        match p.source {
                            Source::Range(start, close) => {
                                Source::Range(start, close - len_utf8!('(' => 1))
                            }
                        },
                    )),
                    SexprType::Stdin => args.push(p.remap(Arg::Stdin)),
                    SexprType::PipedStdin => args.push(p.remap(Arg::PipedStdin)),
                    SexprType::Pipe => args.push(p.remap(Arg::Pipe)),
                    SexprType::Reference(x) => args.push(p.remap(Arg::Reference(x))),
                    SexprType::NewFunction => {} // Skip
                }
            }
        }
        //for a in &self.buffer {
        //    println!(".-> {:?}", a);
        //}
        let parameter_start = start
            + self.buffer[start..]
                .iter()
                .rposition(|t| matches!(t.me, SexprType::NewFunction))
                .unwrap_or(0);

        //let mut is_piped = false;

        let mut output_id = self.output.len();

        // Break up a long statement into its individual s-exprs
        //
        // This is where we would do pratt parsing for order of operations
        // but there is only one in-fix operator, '=' in the language
        while let Some(i) = self.buffer[parameter_start..]
            .iter()
            .rposition(|t| matches!(t.me, SexprType::Assign))
        {
            parse_push_s_args(&mut self.args, self.buffer.drain(parameter_start + i + 1..));
            self.output.push(Sexpr {
                cell_id,
                args: self.args_cursor.move_to(self.args.len()),
                out: output_id,
            });

            self.args
                .push(self.buffer.pop().unwrap().remap(Arg::Assign));
            self.buffer.push(Token::new(
                SexprType::Reference(output_id),
                Source::Range(0, 0),
            ));
            // Push assign off for next s-expr because for `a = b + 1`, we
            // want `b + 1` then `a = <result>`
            output_id += 1;
        }

        // The simple case of determing args_range would be to just calculate
        // `self.args.len()` before and after `parse_push_s_args()`
        parse_push_s_args(&mut self.args, self.buffer.drain(parameter_start..));
        // But {self.args_cursor} allows us to push the first {Arg::Stdin} when
        // initialising the {fsm} and the {Arg::Assign} in above while-loop out
        // in a different order

        self.output.push(Sexpr {
            cell_id,
            args: self.args_cursor.move_to(self.args.len()),
            out: output_id,
        });
        //self.args.push(Token::new(Arg::Output, Source::Range(0, 0)));
        Token::new(SexprType::Reference(output_id), Source::Range(0, 0))
    }
}

use std::vec::Drain;

// {cell_id} increments everytime we arrive a the head or body, i.e. the head
//  and body differ in id by 1
#[derive(Clone, Debug)]
pub struct Sexpr {
    pub cell_id: usize, // for use in determining the STDIN
    pub args: (usize, usize),
    pub out: usize,
}

impl Sexpr {
    pub fn to_display(&self, args: &[Token<Arg>], debug_source: &str) -> String {
        let mut display = format!("({}): (", self.cell_id);
        for arg in &args[self.args.0..self.args.1] {
            display.push_str(&arg.to_display(debug_source));
            display.push_str(", ");
        }
        display.push_str(&format!(") -> {};", self.out));
        display
    }
}

impl Token<Arg> {
    pub fn to_display(&self, source: &str) -> String {
        match self.me {
            Arg::Str => format!("{:?}", self.to_str(source)),
            Arg::Char(c) => format!("{:?}", c),
            Arg::Assign => '='.to_string(),
            // This is either a variable or function identifier
            Arg::Ident => self.to_str(source).to_string(),
            Arg::IdentFunc => self.to_str(source).to_string(),
            Arg::Stdin => '.'.to_string(),
            Arg::PipedStdin => ".|>".to_string(),
            // Temp variables for the output of concats, functions, etc.
            Arg::Reference(i) => format!("{{{}}}", i),
            Arg::Pipe => " |> ".to_string(), // Borrow f# syntax
        }
    }
}
