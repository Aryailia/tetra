//run: cargo test -- --nocapture

//#![feature(round_char_boundary)]

use std::borrow::Cow;

mod walker;
pub use walker::Walker;
mod asciidoctor;
use asciidoctor::AsciiDoctor;
mod commonmark;
use commonmark::CommonMark;

#[derive(Debug)]
pub struct Metadata<'a> {
    pub outline: Vec<(u8, Cow<'a, str>)>,
    pub links: Vec<(Cow<'a, str>, Cow<'a, str>)>,
}

// Defines 'pub enum FileType' and 'FROM_EXT'
include!(concat!(env!("OUT_DIR"), "/codegen.rs"));

impl FileType {
    pub const fn id(&self) -> usize {
        // SAFETY: The representation of $enum is set
        unsafe { *(self as *const Self as *const usize) }
    }

    pub fn from(extension: &str) -> Self {
        FROM_EXT
            .get(extension)
            .copied()
            .unwrap_or(FileType::Default)
    }
}

pub trait Analyse {
    fn comment_prefix(&self) -> &'static str { todo!("Unknown line comment symbol.") }
    fn metadata<'a>(&self, _source: &'a str) -> Metadata<'a> {
        todo!()
    }

}

impl Analyse for FileType {
    fn comment_prefix(&self) -> &'static str { VTABLE[self.id()].comment_prefix() }
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
        assert_eq!("//", FileType::from("adoc").comment_prefix());
        //FileType::AsciiDoctor.metadata("= Yo");
    }
}
