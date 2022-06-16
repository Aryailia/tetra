//run: cargo test -- --nocapture
pub mod lexer;
pub mod sexpr;
pub mod ast;

use std::fmt;
use crate::framework::Token;

fn unwrap<T, E: fmt::Display>(original: &str, result: Result<T, Token<E>>) -> T {
    match result {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{} {}", e.get_context(original), e.me);
            std::process::exit(1);
        }
    }
}


pub fn parse(content: &str) -> ast::ParseOutput {
    let lexemes = unwrap(content, lexer::process(content, true));
    let (sexprs, args) = unwrap(content, sexpr::process(&lexemes, content));
    unwrap(content, ast::process(&sexprs, &args))
}

