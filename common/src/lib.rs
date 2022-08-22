//run: cargo test -- --nocapture

//#![feature(round_char_boundary)]

use std::borrow::Cow;
use std::collections::HashMap;

mod walker;
pub use walker::Walker;
mod filetype;
use filetype::*;
mod metadata;

#[derive(Debug)]
pub struct Metadata<'a> {
    pub outline: Vec<(u8, Cow<'a, str>)>,
    pub links: Vec<(Cow<'a, str>, Cow<'a, str>)>,
    pub attributes: HashMap<&'a str, &'a str>,
}

//run: cargo test -- --nocapture

// NOTE: I am trying to remove dependencies on syn, thus the perfect hash
//       is created not from a proc_macro but from 'build.rs' which is then
//       imported here
// Defines 'pub enum FileType' and 'FROM_EXT'
include!(concat!(env!("OUT_DIR"), "/codegen.rs"));

impl FileType {
    pub const fn id(&self) -> usize {
        // SAFETY: The representation of $enum is set, see source in 'build.rs'
        unsafe { *(self as *const Self as *const usize) }
    }

    pub fn from(extension: &str) -> Option<Self> {
        FROM_EXT
            .get(extension)
            .copied()
    }
}

pub trait Analyse {
    fn comment_prefix(&self) -> &'static str { todo!("Unknown line comment symbol.") }
    fn comment_suffix(&self) -> &'static str { todo!("Unknown line comment symbol.") }
    fn metadata<'a>(&self, _source: &'a str) -> Metadata<'a> {
        todo!() // Default panic if not overridden
    }

}

impl Analyse for FileType {
    fn comment_prefix(&self) -> &'static str { VTABLE[self.id()].comment_prefix() }
    fn comment_suffix(&self) -> &'static str { VTABLE[self.id()].comment_suffix() }
    fn metadata<'a>(&self, source: &'a str) -> Metadata<'a> {
        VTABLE[self.id()].metadata(source)
    }
}

struct Todo();
impl Analyse for Todo {}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!("//", FileType::from("adoc").unwrap().comment_prefix());
        let file = std::fs::read_to_string("../readme-source.md").unwrap();
        //println!("{}", FileType::CommonMark.metadata(&file).to_json());
    }
}
