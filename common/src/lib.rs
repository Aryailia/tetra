//run: cargo test -- --nocapture

//#![feature(round_char_boundary)]

use std::borrow::Cow;

mod walker; pub use walker::Walker;
mod asciidoctor; use asciidoctor::AsciiDoctor;
mod commonmark; use commonmark::CommonMark;


#[derive(Debug)]
pub struct Metadata<'a> {
    pub outline: Vec<(u8, Cow<'a, str>)>,
    pub links: Vec<(Cow<'a, str>, Cow<'a, str>)>,
}

// 'enum_dispatch' might be useful
macro_rules! declare {
    ($enum:ident, $( $variant:ident : $struct:ident $(, $val:expr)*; )*) => {

        #[derive(Debug)]
        #[repr(usize)]
        pub enum $enum {
            $( $variant, )*
        }

        const VTABLE: [&dyn Analyse; 0 $( + { _ = $enum::$variant; 1 } )*] = [
            $( &$struct(), )*
        ];

        impl $enum {
            pub const fn id(&self) -> usize {
                // SAFETY: The representation of $enum is set
                unsafe { *(self as *const Self as *const usize) }
            }
        }

        pub trait Analyse {
            fn metadata<'a>(&self, _source: &'a str) -> Metadata<'a> { todo!() }
        }

        impl Analyse for $enum {
            fn metadata<'a>(&self, source: &'a str) -> Metadata<'a> {
                VTABLE[self.id()].metadata(source)
            }
        }
    };
}

declare! {FileType,
    AsciiDoctor: AsciiDoctor;
    //Markdown;
    CommonMark: CommonMark;
}

struct Todo();
impl Analyse for Todo{}


#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn it_works() {
        //FileType::AsciiDoctor.metadata("= Yo");
    }
}
