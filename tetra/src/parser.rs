//run: cargo test -- --nocapture
mod ast;
mod lexer;
mod sexpr;

pub use sexpr::SexprOutput;
pub use ast::{AstOutput, Command};

pub use lexer::process as step1_lex;
pub use sexpr::process as step2_to_sexpr;
pub use ast::process as step3_to_ast;

use crate::framework::Token;


// @TODO: remove clone from these enums

// The elements of an s-expr. The syntaxemes. For "sexpr.rs"
// We only want `derive(Eq)` for the test in 'lib.rs'. '--test' runs in debug
#[derive(Clone, Debug)]
#[cfg_attr(debug_assertions, derive(PartialEq, Eq))]
pub enum Item {
    Str,
    Text(&'static str),
    Ident,      // Variable or function
    Func,       // Item::Ident that was narrowed down to only a function
    Stdin,      // Referes to the associated heredoc body
    Assign,     // If we add more infix operators, consider 'Item::Infix(char)'
    Pipe,       //
    PipedStdin, // 'Item::Stdin' and 'Item::Pipe' combined, the beginning of heredoc headers
    // @TODO: Check if this has to be different from Arg::Stdin
    //        I expect this to catch ". cite" expressions
    Reference(usize), // Id that should match {Sexpr.out}. Index into {SexprOutput.0}
    Concat,

    // The following are just for parsing, but are not included in args {SexprOutput.1}
    Comma,

    // The following should never be pushed into {to_process}.
    // These are for the {balance} tracking, so should never be printed.
    Paren,
    Stmt,
}

// 'Item' but trimmed down to just the types. For "ast.rs" and "run/*".
#[derive(Clone, Debug)]
pub enum Param {
    Str,
    Literal(&'static str),
    Ident,
    Func,
    Reference(usize),
}

#[derive(Clone, Debug)]
#[cfg_attr(debug_assertions, derive(PartialEq, Eq))]
pub enum Label {
    Assign, // "<l-value> = <r-value>"
    Concat, // Just display all the arguments as is
    Ident,  // Variable lookup or a function call
    Func,   // Function call
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


