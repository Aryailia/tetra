//run: cargo test -- --nocapture
mod ast;
mod lexer;
mod sexpr;

pub use sexpr::SexprOutput;
pub use ast::{AstOutput, Command};

pub use lexer::process as step1_lex;
pub use sexpr::process as step2_to_sexpr;
pub use ast::process as step3_to_ast;

////////////////////////////////////////////////////////////////////////////////

use std::fmt::Write as _; // clippy: import without risk of name clashing

use crate::framework::Token;

////////////////////////////////////////////////////////////////////////////////

// @TODO: remove clone from these enums

// The elements of an s-expr. The syntaxemes. For "sexpr.rs"
// We only want `derive(Eq)` for the test in 'lib.rs'. '--test' runs in debug
#[derive(Debug)]
#[cfg_attr(debug_assertions, derive(PartialEq, Eq))]
pub enum Item {
    Str,
    Literal(&'static str),
    Reference(usize), // Id that should match {Sexpr.out}. Index into {SexprOutput.0}
    Ident,            // Variable or function
    Func,             // Item::Ident that was narrowed down to only a function
    Key,              // For optional arguments, i.e. the key of key-value pairs

    Stdin,      // Referes to the associated heredoc body
    Pipe,       //
    PipedStdin, // 'Item::Stdin' and 'Item::Pipe' combined, the beginning of heredoc headers
    Assign,     // If we add more infix operators, consider 'Item::Infix(char)'
    // @TODO: Check if this has to be different from Arg::Stdin
    //        I expect this to catch ". cite" expressions
    Concat,

    // The following are just for parsing, but are not included in args {SexprOutput.1}
    Colon,
    Comma,

    // The following should never be pushed into {to_process}.
    // These are for the {balance} tracking, so should never be printed.
    Paren,
    Stmt,
}

// After the ast.rs pass, we trim down to this
// 'Item' but trimmed down to just the types. For "ast.rs" and "run/*".
#[derive(Clone, Debug)]
pub enum Param {
    Str,
    Literal(&'static str),
    Reference(usize),
    Ident,
    Key,
}

// 'Item' but trimmed down to just what can be a function label
#[derive(Clone, Debug)]
#[cfg_attr(debug_assertions, derive(PartialEq, Eq))]
pub enum Label {
    Assign, // "<l-value> = <r-value>"
    Concat, // Just display all the arguments as is
    Ident,  // Variable lookup or a function call
    Func,   // Function call
}


impl Token<Item> {
    pub fn push_display(&self, buffer: &mut String, source: &str) {
        match self.me {
            Item::Str => write!(buffer, "{:?}", self.to_str(source)).unwrap(),
            Item::Literal(s) => write!(buffer, "{:?}", s).unwrap(),
            Item::Assign => buffer.push('='),
            // This is either a variable or function identifier
            Item::Ident => buffer.push_str(self.to_str(source)),
            Item::Key => {
                buffer.push_str(self.to_str(source));
                buffer.push(':');
            }
            Item::Func => {
                buffer.push_str(self.to_str(source));
                buffer.push('(');
            }
            Item::Stdin => buffer.push('.'),
            // Temp variables for the output of concats, functions, etc.
            Item::Reference(i) => write!(buffer, "{{{}}}", i).unwrap(),
            Item::Concat => buffer.push_str("#Concat("),

            Item::Pipe => buffer.push('|'),
            Item::PipedStdin => buffer.push_str(". | "),

            //Item::Comma => buffer.push_str("\\,"),
            Item::Colon | Item::Comma => unreachable!(),
            Item::Paren | Item::Stmt => unreachable!(),
        }
    }

    pub fn print(&self, source: &str) {
        let mut buffer = String::new();
        self.push_display(&mut buffer, source);
        print!("{}", buffer);
    }
}

impl Token<Param> {
    pub fn push_display(&self, buffer: &mut String, source: &str) {
        match self.me {
            Param::Str => write!(buffer, "{:?}", self.to_str(source)).unwrap(),
            Param::Literal(s) => write!(buffer, "{:?}", s).unwrap(),
            Param::Ident| Param::Key => buffer.push_str(self.to_str(source)),
            Param::Reference(i) => write!(buffer, "{{{}}}", i).unwrap(),
        }
    }
}

impl Token<Label> {
    pub fn push_display(&self, buffer: &mut String, original: &str) {
        match self.me {
            Label::Assign => buffer.push('='),
            Label::Concat => buffer.push_str("#Concat"),
            Label::Func => {
                buffer.push_str(self.to_str(original));
                buffer.push('(');
            }
            Label::Ident => buffer.push_str(self.to_str(original)),
        }
    }
}


// TODO: Mostly a reminder that we want to pre-allocate everything the parser
//       needs because it is possible and probably makes it faster.
//       Unsure if this would be worth it though.
//
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


