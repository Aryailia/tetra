//run: cargo test -- --nocapture

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
    let output_token = fsm.drain_push_sexpr(cell_id);
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
                let _output_token = fsm.drain_push_sexpr(cell_id);
                cell_id += 1;

                // Do not push the concat for the heredoc body since the output
                // is pushed onto {knit_sexpr} on the LexType::HereDocClose
                //fsm.buffer.push(output_token);
                //knit_sexpr.push(output_token);

                // @TODO: Double check that we have to push SexprType::NewFunction
                // We push SexprType::NewFunction for multiple commands in the
                // code body, e.g. "{| cite ''; print() |}"
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
            (Mode::Code, LexType::Pipe) => {
                let output_token = fsm.drain_push_sexpr(cell_id);
                fsm.buffer.push(output_token);
                fsm.buffer.push(Token::new(SexprType::Pipe, source));
            }
            (Mode::Code, LexType::HereDocClose) => {
                fsm.mode = Mode::Text;
                //fsm.drain_push_sexpr(debug_source);
                let output_token = fsm.drain_push_sexpr(cell_id);
                knit_sexpr.push(output_token);
                cell_id += 1;
                //fsm.buffer.push(output_token);
            }
            (Mode::Code, LexType::InlineClose) => {
                fsm.mode = Mode::Text;
                let output_token = fsm.drain_push_sexpr(cell_id);
                fsm.buffer.push(output_token);
            }

            (Mode::Code, LexType::QuoteStart) => {
                fsm.mode = Mode::Quote;
                fsm.buffer.push(Token::new(SexprType::NewFunction, source));
            }
            (Mode::Code, LexType::CmdSeparator) => {
                // "display ''; cite" means we ignore the output of the first command
                let _output_token = fsm.drain_push_sexpr(cell_id);
                fsm.buffer.push(Token::new(SexprType::NewFunction, source));
            }
            (Mode::Code, LexType::ParenStart) => {} // @TODO
            (Mode::Code, LexType::ParenClose) => {} // @TODO
            (Mode::Code, _) => return Err(Token::new("Unhandled token", source)),
            //(Mode::Code, _) => debug_print_token!(die@l, debug_source),

            ////////////////////////////////////////////////////////////////////
            (Mode::Quote, LexType::Quoted) => {
                fsm.buffer.push(Token::new(SexprType::Str, source));
            }
            (Mode::Quote, LexType::QuoteEscaped(c)) => {
                fsm.buffer.push(Token::new(SexprType::Char(*c), source));
            }
            (Mode::Quote, LexType::QuoteClose) => {
                fsm.mode = Mode::Code;
                let output_token = fsm.drain_push_sexpr(cell_id);
                fsm.buffer.push(output_token);
            }
            (Mode::Quote, _) => debug_print_token!(die@l, debug_source),
        }
    }

    // Push the final heredoc body as a concat-display command
    // Model this after LexType::HereDocStart branch of match
    let _output_token = fsm.drain_push_sexpr(cell_id);
    // Do not push this {_output_token} into the buffer
    fsm.buffer.extend(knit_sexpr);
    fsm.drain_push_sexpr(cell_id + 1);

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
    Unknown, // Range source
    Reference(usize), // Index into {args} pointing to an 'Arg::Output'
    Stdin,
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
    Stdin,
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

    fn drain_push_sexpr(&mut self, cell_id: usize) -> Token<SexprType> {
        //for a in &self.buffer {
        //    println!(".-> {:?}", a);
        //}
        let parameter_start = self
            .buffer
            .iter()
            .rposition(|p| matches!(p.me, SexprType::NewFunction))
            .unwrap_or(0);
        let parameters = self.buffer.drain(parameter_start..);
        //let mut is_piped = false;

        for p in parameters {
            //debug_print_token!(p, debug_source);
            match p.me {
                SexprType::Str => self.args.push(p.remap(Arg::Str)),
                SexprType::Char(c) => self.args.push(p.remap(Arg::Char(c))),
                SexprType::Ident => self.args.push(p.remap(Arg::Unknown)),
                SexprType::Stdin => self.args.push(p.remap(Arg::Stdin)),
                SexprType::PipedStdin => self.args.push(p.remap(Arg::PipedStdin)),
                SexprType::Pipe => self.args.push(p.remap(Arg::Pipe)),
                SexprType::Reference(x) => self.args.push(p.remap(Arg::Reference(x))),
                SexprType::NewFunction => {} // Skip
            }
        }

        let args_post_index = self.args.len();
        let sexpr_index = self.output.len();
        let sexpr = Sexpr {
            cell_id,
            args: self.args_cursor.move_to(args_post_index),
            out: sexpr_index,
        };
        //self.args.push(Token::new(Arg::Output, Source::Range(0, 0)));
        self.args_cursor.move_to(args_post_index);

        self.output.push(sexpr);

        //for a in &self.buffer {
        //    println!("- {:?}", a);
        //}
        // @TODO: range of all args?
        Token::new(SexprType::Reference(sexpr_index), Source::Range(0, 0))
    }

    //fn print_command(&self, cmd: &Sexpr, debug_source: &str) {
    //    print!("Command: (");
    //    for arg in &self.args[cmd.0.0..cmd.0.1] {
    //        match arg {
    //            Arg::Str(s) => print!("{:?}", s.to_str(debug_source)),
    //            Arg::Char(c, _) => print!("{:?}", c),
    //            Arg::Unknown(s) => print!("{{{}}}", s.to_str(debug_source)),
    //            Arg::Stdin => print!("."),
    //            Arg::PipedStdin => print!(".>"),
    //            Arg::Reference(i) => print!("{{{}}}", i),
    //            Arg::Pipe => print!(" |> "),
    //        }
    //        print!(", ");
    //    }
    //    println!(");");
    //}
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
            // This is either a variable or function identifier
            Arg::Unknown => self.to_str(source).to_string(),
            Arg::Stdin => '.'.to_string(),
            Arg::PipedStdin => ".|>".to_string(),
            // Temp variables for the output of concats, functions, etc.
            Arg::Reference(i) => format!("{{{}}}", i),
            Arg::Pipe => " |> ".to_string(), // Borrow f# syntax
        }
    }
}
