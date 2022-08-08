//run: cargo test -- --nocapture

//#![feature(round_char_boundary)]

use std::fmt::Write as _;
use std::borrow::Cow;

use tinyjson::JsonValue;

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

impl<'a> Metadata<'a> {
    pub fn to_json(&self) -> String {
        let mut buffer = String::new();
        buffer.push_str("{\"outline\":[");

        let mut iter = self.outline.iter();
        if let Some((level, body)) = iter.next() {
            let body_json = JsonValue::String(body.to_string()).stringify().unwrap();
            write!(&mut buffer, "[{},{}]", level, body_json).unwrap();
            for (level, body) in iter {
                let body_json = JsonValue::String(body.to_string()).stringify().unwrap();
                write!(&mut buffer, ",[{},{}]", level, body_json).unwrap();
            }
        }

        buffer.push_str("],\"links\":[");

        let mut iter = self.links.iter();
        if let Some((uri, body)) = iter.next() {
            let uri_json = JsonValue::String(uri.to_string()).stringify().unwrap();
            let body_json = JsonValue::String(body.to_string()).stringify().unwrap();
            write!(&mut buffer, "[{},{}]", uri_json, body_json).unwrap();
            for (uri, body) in iter {
                let uri_json = JsonValue::String(uri.to_string()).stringify().unwrap();
                let body_json = JsonValue::String(body.to_string()).stringify().unwrap();
                write!(&mut buffer, ",[{},{}]", uri_json, body_json).unwrap();
            }
        }
        buffer.push_str("]}");
        buffer
    }
}

// Defines 'pub enum FileType' and 'FROM_EXT'
include!(concat!(env!("OUT_DIR"), "/codegen.rs"));

impl FileType {
    pub const fn id(&self) -> usize {
        // SAFETY: The representation of $enum is set
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
        assert_eq!("//", FileType::from("adoc").unwrap().comment_prefix());
        let file = std::fs::read_to_string("../readme-source.md").unwrap();
        FileType::CommonMark.metadata(&file).to_json();
        //FileType::AsciiDoctor.metadata("= Yo");
    }
}
