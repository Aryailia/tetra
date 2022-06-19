//run: cargo test -- --nocapture
mod ast;
mod lexer;
mod sexpr;

pub use sexpr::Arg;
pub use ast::{Command, Label};

pub use lexer::process as step1_lex;
pub use sexpr::process as step2_to_sexpr;
pub use ast::process as step3_to_ast;
