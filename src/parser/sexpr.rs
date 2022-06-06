//run: cargo test -- --nocapture

// This decides the grouping of lexemes into commands as well as breaking up
// multi-statement commands into single commands.
//     e.g. '{$ a = b = c $}' -> 'b = c' and 'a = <output>'
//
// Idents are pushed out to their own s-expr,
//     e.g. '{| cite(1, len, 3) |}' -> `(len)` and `(cite, 1, <ref-to-len>, 3)`
//          as oppose to  `(cite, len)`
// This is because we do not know if 'len' is a variable or a function ident.
// This means in general we have too many s-expr emitted, but we cannot resolve
// this ambiguity until we know the function list or execute variable assigns.
// And we do our best to optimise these extra s-exprs away in the ast.rs step.


use super::lexer::LexType;
use crate::framework::{Source, Token};

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
// - no identifier is marked as the function name yet
// - STDIN arguments are not resolved to which body their refer
//
// Additionally, we haven't discriminated variable and function identifiers at
// this stage yet. E.g. "cite len a" might all be functions.

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

    let mut fsm = Fsm::new(lexemes.len(), debug_source);
    //for l in lexemes {
    //    debug_print_token!(l, debug_source);
    //}

    let mut knit_sexpr = Vec::new(); // For the final document concat
    let mut cell_id = 0; // HereDoc headers and bodies get different id's
    let buffer = &mut Vec::with_capacity(lexemes.len());

    // We act as if all documents start with an invisible heredoc at the start
    // Model after the actions of 'LexType::HereDocClose' branch
    fsm.args.push(Token::new(Arg::Stdin, Source::Range(0, 0)));
    let output_token = fsm.drain_push_sexpr(buffer, cell_id, 0)?;
    knit_sexpr.push(output_token);
    cell_id += 1;

    for l in lexemes {
        //debug_print_token!(l, debug_source);
        ////let stdin = 0;
        let source = l.source.clone();
        match (&fsm.mode, &l.me) {
            (Mode::Text, LexType::Text) => {
                //println!("{:?}", l.to_str(debug_source));
                buffer.push(l.remap(SexprType::Str));
            }
            (Mode::Text, LexType::BlockComment) => {} // Skip comments
            (Mode::Text, LexType::HereDocStart) => {
                fsm.mode = Mode::Code;
                let _output_token = fsm.drain_push_sexpr(buffer, cell_id, 0)?;
                cell_id += 1;

                // @TODO: Double check that we have to push 'SexprType::NewFunction'
                // We push 'SexprType::NewFunction' for multiple commands in the
                // code body, e.g. "{| cite ''; print() |}"

                // 'LexType::HereDocStart' indicates the previous cell has
                // fully outputted all its s-exprs so we can empty {buffer}
                buffer.clear();
                // Do not need to demarcate a new s-expr because we clear
                //buffer.push(Token::new(SexprType::NewFunction, source));
                buffer.push(Token::new(SexprType::PipedStdin, source));
            }
            (Mode::Text, LexType::InlineStart) => {
                fsm.mode = Mode::Code;
                buffer.push(Token::new(SexprType::NewFunction, source));
            }
            (Mode::Text, _) => debug_print_token!(die@l, debug_source),

            ////////////////////////////////////////////////////////////////////
            (Mode::Code, LexType::BlockComment) => debug_print_token!(die@l, debug_source),
            (Mode::Code, LexType::Stdin) => {
                buffer.push(Token::new(SexprType::Stdin, source));
                //args.push(Token::new());
            }
            (Mode::Code, LexType::Ident) => {
                buffer.push(Token::new(SexprType::Ident, source));
            }
            (Mode::Code, LexType::IdentParen) => {
                buffer.push(Token::new(SexprType::IdentFunc, source));
                //fsm.stack.push(buffer.len());
            }
            (Mode::Code, LexType::Pipe) => {
                let output_token = fsm.drain_push_sexpr(buffer, cell_id, 0)?;
                buffer.push(output_token);
                buffer.push(Token::new(SexprType::Pipe, source));
            }
            (Mode::Code, LexType::HereDocClose) => {
                fsm.mode = Mode::Text;
                let output_token = fsm.drain_push_sexpr(buffer, cell_id, 0)?;
                knit_sexpr.push(output_token);
                cell_id += 1;
                //buffer.push(output_token);
            }
            (Mode::Code, LexType::InlineClose) => {
                fsm.mode = Mode::Text;
                let output_token = fsm.drain_push_sexpr(buffer, cell_id, 0)?;
                buffer.push(output_token);
            }

            (Mode::Code, LexType::QuoteStart) => {
                fsm.mode = Mode::Quote;
                buffer.push(Token::new(SexprType::NewFunction, source));
            }
            (Mode::Code, LexType::CmdSeparator) => {
                // "display ''; cite" means we ignore the output of the first command
                let _output_token = fsm.drain_push_sexpr(buffer, cell_id, 0)?;
                buffer.push(Token::new(SexprType::NewFunction, source));
            }
            // After lexing, open parenthesis mean a function call
            //(Mode::Code, LexType::ParenStart) => {
            //    //let func_ident = buffer.pop().unwrap();
            //    //debug_assert!(matches!(func_ident.me, SexprType::Ident));
            //    ////println!("{:?}", buffer);
            //    //buffer.push(Token::new(SexprType::NewFunction, source));
            //    //buffer.push(func_ident);
            //}
            (Mode::Code, LexType::ParenClose) => {
                // if(cite a)
                // if cite(a)
                //let output_token = fsm.drain_push_sexpr(buffer, cell_id, 0)?;
                //buffer.push(output_token);
                let i = buffer
                    .iter()
                    .rposition(|t| matches!(t.me, SexprType::IdentFunc))
                    .unwrap();

                // Push the interior of the parenthesis if not empty
                // e.g. `cite(ref 1)` -> push `ref 1`
                if i + 1 < buffer.len() {
                    let output_token = fsm.drain_push_sexpr(buffer, cell_id, i + 1)?;
                    buffer.push(output_token);
                }

                // If there was a pipe then process normally
                if matches!(buffer[i - 1].me, SexprType::Pipe | SexprType::PipedStdin) {
                    let output_token = fsm.drain_push_sexpr(buffer, cell_id, 0)?;
                    buffer.push(output_token);
                // Otherwise we are nested within another function call and
                // only process the function itself
                // e.g. `cite(ref 1)` -> push `cite(<output>)` after the first push
                } else {
                    let output_token = fsm.drain_push_sexpr(buffer, cell_id, i)?;
                    buffer.push(output_token);
                    //buffer.push(Token::new(SexprType::NewFunction, source));
                }
            }
            (Mode::Code, LexType::Assign) => {
                buffer.push(Token::new(SexprType::Assign, source));
            }
            (Mode::Code, LexType::ArgSeparator) => {
                buffer.push(l.remap(SexprType::ArgSeparator));
                //buffer.push
            }
            (Mode::Code, _) => return Err(Token::new("Sexpr.rs: Unhandled token", source)),
            //(Mode::Code, _) => debug_print_token!(die@l, debug_source),

            ////////////////////////////////////////////////////////////////////
            // @TODO: What should happen with quotes in succession without
            //        whitespace separator e.g. `cite "jane"'doe'`
            (Mode::Quote, LexType::Quoted) => {
                buffer.push(Token::new(SexprType::Str, source));
            }
            (Mode::Quote, LexType::QuoteEscaped(c)) => {
                buffer.push(Token::new(SexprType::Char(*c), source));
            }
            (Mode::Quote, LexType::QuoteClose) => {
                fsm.mode = Mode::Code;
                let output_token = fsm.drain_push_sexpr(buffer, cell_id, 0)?;
                buffer.push(output_token);
            }
            (Mode::Quote, _) => debug_print_token!(die@l, debug_source),
        }
    }

    // Push the final heredoc body as a concat-display command
    // Model this after LexType::HereDocStart branch of match
    let _output_token = fsm.drain_push_sexpr(buffer, cell_id, 0)?;
    // Do not push this {_output_token} into the buffer
    buffer.extend(knit_sexpr);
    fsm.drain_push_sexpr(buffer, cell_id + 1, 0)?;

    //for p in buffer {
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
}

#[derive(Debug)]
enum SexprType {
    Str,        // Index into {data} array
    Char(char), // Index into {data} array
    Ident,
    IdentFunc,
    Stdin,
    Assign,

    ArgSeparator,

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
    stack: Vec<usize>, // @TODO: for paraenthesis and bracket balancing
    debug_source: String,
}

impl Fsm {
    fn new(capacity: usize, debug_source: &str) -> Self {
        // Parsing into sexpr will never produce more than the number of lexemes
        // as we never push
        Self {
            mode: Mode::Text,
            output: Vec::with_capacity(capacity),

            args: Vec::with_capacity(capacity),
            args_cursor: Cursor(0),
            args_stdin_index: 0,

            stdin_range: (0, 0),
            stack: Vec::with_capacity(capacity),
            debug_source: debug_source.to_string(),
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
    fn drain_push_sexpr(
        &mut self,
        buffer: &mut Vec<Token<SexprType>>,
        cell_id: usize,
        start: usize,
    ) -> Result<Token<SexprType>, ParseError> {
        //for a in &self.buffer {
        //    println!(".-> {:?}", a);
        //}
        let parameter_start = start
            + buffer[start..]
                .iter()
                .rposition(|t| matches!(t.me, SexprType::NewFunction))
                .unwrap_or(0);

        // Break up a long statement into its individual s-exprs
        // Currently, we can only construct multi-expression commands with
        // the inline operators, of which, there is only the assign operator.
        //
        // This is where we would do pratt parsing for order of operations
        // but there is only one in-fix operator, '=' in the language
        let mut arg_buffer = Vec::new();
        while let Some(i) = buffer[parameter_start..]
            .iter()
            .rposition(|t| matches!(t.me, SexprType::Assign))
        {
            let (output_id, source) =
                self.parse_push_s_args(cell_id, &mut arg_buffer, buffer, parameter_start + i + 1)?;

            // The while sentinel ensures this is an SexprType::Assign
            let assign = buffer.pop().unwrap();
            debug_assert!(matches!(assign.me, SexprType::Assign));
            self.args.push(assign.remap(Arg::Assign));

            buffer.push(Token::new(SexprType::Reference(output_id), source));
            // Push assign off for next s-expr because for `a = b + 1`, we
            // want `b + 1` then `a = <result>`
        }

        let (output_id, source) =
            self.parse_push_s_args(cell_id, &mut arg_buffer, buffer, parameter_start)?;
        //self.args.push(Token::new(Arg::Output, Source::Range(0, 0)));
        Ok(Token::new(SexprType::Reference(output_id), source))
    }

    // To be used by `parse_push_s_args()`
    // Idents could either be variable lookups or functions
    // This pushes out all the idents first as single-argument s-exprs as
    //
    //
    // Push the parameters into {args}, breaking out idents into their
    // own s-expr (if not the first argument) and re-ordering the piped
    // value to be pushed last
    fn parse_push_s_args(
        &mut self,
        cell_id: usize,

        // We push to a buffer, which then gets pushed to {self.args}
        // So that single-argument idents can be pushed before
        // the current command, so that {self.args_cursor} tracks properly
        buffer: &mut Vec<Token<Arg>>,
        params: &mut Vec<Token<SexprType>>,
        start: usize,
    ) -> Result<(usize, Source), ParseError> {

        let mut comma_after_first = false;
        {
            let mut first: Option<Arg> = None;
            // {x} is the parameter index including the label but no including
            // piped arguments as these are to be pushed to the end
            // {y} is the same as {x} but tracks how many arguments since the
            // last 'SexprType::ArgSeparator', i.e. since the last comma
            let (mut x, mut y) = (0, 0);
            for p in &params[start..] {
                (x, y, first) = match p.me {
                    SexprType::Str => (x + 1, y + 1, first.or(Some(Arg::Str))),
                    SexprType::Char(_) => (x + 1, y + 1, first.or(Some(Arg::Str))),
                    SexprType::Assign => unreachable!(),
                    SexprType::ArgSeparator if x == 1 => {
                        comma_after_first = true;
                        // The only place {x} and {y} are different
                        // This is because {y} is to count how many arguments
                        // since the last 'SexprType::ArgSeparator'
                        (x, 0, first)
                    }
                    SexprType::ArgSeparator => (x, 0, first),
                    SexprType::Ident => (x + 1, y + 1, first.or(Some(Arg::Ident))),
                    SexprType::IdentFunc => (x + 1, y + 1, first.or(Some(Arg::Ident))),
                    SexprType::Stdin => (x + 1, y + 1, first),

                    // 'SexprType::PipedStdin' and 'SexprType::Pipe'
                    SexprType::PipedStdin if x == 0 => (0, 0, first),
                    SexprType::PipedStdin => return Err(p.remap("sexpr.rs: asdf")),
                    // Because our parent function `parse_push_s_args()`
                    SexprType::Pipe if x == 1 => (0, 0, first),
                    SexprType::Pipe => return Err(p.remap("sexpr.rs: aalskdjfalkdsjf")),
                    SexprType::Reference(_) => (x + 1, y + 1, first),
                    SexprType::NewFunction => (0, 0, first),
                };

                if y >= 3 && matches!(first, Some(Arg::Ident)) {
                    return Err(p.remap("sexpr.rs: Need a comma to separate arguments"));
                }
            }
        }

        // Should be guarenteed from main `match` branching
        debug_assert!(
            1 >= params[start..]
                .iter()
                .filter(|p| matches!(p.me, SexprType::PipedStdin | SexprType::Pipe))
                .count()
        );

        // Re-order piped values to push to the end, and push out any idents
        // that are not labels as their own s-expr. The pushed out idents could
        // be variable or function idents.
        let mut piped_arg = None;
        for p in params.drain(start..) {
            //debug_print_token!(p, &self.debug_source);
            //println!("{:?} {:?}", param_count, p);

            match p.me {
                SexprType::Str => buffer.push(p.remap(Arg::Str)),
                SexprType::Char(c) => buffer.push(p.remap(Arg::Char(c))),
                SexprType::Assign => unreachable!(),
                SexprType::ArgSeparator => {}
                SexprType::Ident => {
                    // '{| cite(<ident>, a) |}' vs `{| cite(<ident> a) |}`
                    // one s-expr vs two s-exprs

                    // If we are the first argument and there is no
                    // {comma_after_first}, then the current {p} is the
                    // label for the command that {params.drain(start..)} is
                    if buffer.is_empty() && !comma_after_first {
                        // @TODO: add function call if second argument is a
                        //debug_assert_eq!(param_count, 1);

                        buffer.push(p.remap(Arg::Ident));

                    // Otherwise we are argument of a command whose label
                    // is specified before the current {p}
                    // e.g. '{| cite(cite(), 1) |}' where {p} is the second cite
                    } else {
                        self.args.push(p.remap(Arg::Ident));
                        let output_id = self.output.len();
                        self.output.push(Sexpr {
                            cell_id,
                            args: self.args_cursor.move_to(self.args.len()),
                            out: output_id,
                        });
                        buffer.push(p.remap(Arg::Reference(output_id)));
                    }
                }

                SexprType::IdentFunc => {
                    // Remove the paren at the end of the source
                    let new_source = match p.source {
                        Source::Range(start, close) => {
                            Source::Range(start, close - len_utf8!('(' => 1))
                        }
                    };

                    if buffer.is_empty() {
                        buffer.push(Token::new(Arg::IdentFunc, new_source));
                    } else {
                        // We are always the label of a command, i.e.
                        // we cannot be a variable ident
                        unreachable!()
                    }
                }
                SexprType::Stdin => buffer.push(p.remap(Arg::Stdin)),
                SexprType::PipedStdin if buffer.is_empty() => {
                    piped_arg = Some(p.remap(Arg::Stdin));
                }
                SexprType::PipedStdin => unreachable!(),
                SexprType::Pipe if buffer.len() == 1 => {
                    piped_arg = buffer.pop();
                    debug_assert!(piped_arg.is_some())
                }
                SexprType::Pipe => unreachable!(),
                SexprType::Reference(x) => buffer.push(p.remap(Arg::Reference(x))),
                SexprType::NewFunction => {}
            }
        }
        if let Some(arg) = piped_arg {
            buffer.push(arg)
        }
        self.args.append(buffer);

        let output_id = self.output.len();
        // The simple case of determing args_range would be to just calculate
        // `self.args.len()` before and after `parse_push_s_args()`
        // But {self.args_cursor} allows us to push the first {Arg::Stdin} when
        // initialising the {fsm} and the {Arg::Assign} in above while-loop out
        // in a different order
        let range = self.args_cursor.move_to(self.args.len());
        let source = source_span(range, &self.args);
        self.output.push(Sexpr {
            cell_id,
            args: range,
            out: output_id,
        });
        Ok((output_id, source))
        //Err(Token::new("sexpr.rs: aalskdjfalkdsjf", Source::Range(0, 0)))
    }
}

fn source_span((start, close): (usize, usize), args: &[Token<Arg>]) -> Source {
    let parameters = &args[start..close];
    debug_assert!(!parameters.is_empty());
    // Would only be empty if we push an s-expr with no arguments, which
    // should not be possible. We already if-statement catch the
    // 'LexType::IdentFunc' case, i.e. a case like '{$ cite() $}'

    let source1 = match parameters.first().map(|p| &p.source) {
        Some(Source::Range(a, _)) => *a,
        _ => unreachable!(), // {parameters} guarenteed to not be empty
    };
    let source2 = match parameters.last().map(|p| &p.source) {
        Some(Source::Range(_, b)) => *b,
        _ => unreachable!(), // {parameters} guarenteed to not be empty
    };
    Source::Range(source1, source2)
}

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
            // Temp variables for the output of concats, functions, etc.
            Arg::Reference(i) => format!("{{{}}}", i),
        }
    }
}
