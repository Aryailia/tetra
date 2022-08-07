//run: cargo test -- --nocapture

// AsciiDoctor supports three methods of titles
// * ATX headers "# Header"
// * Setext headers "Header\n====", or at least their own flavour
// * AsciiDoctor headers "== Header"
// ATX and Setext terminology comes from Markdown/CommonMark
//
// Additionally, AsciiDoctor defines that only the title gets to be <h1>,
// whereas in Markdown, there can be multiple "# Header 1"

use std::borrow::Cow;

use super::{Analyse, Metadata, Walker};

pub struct AsciiDoctor();

// Just a dirty solution that only handles 'link:url.com/[]' type links
// and handles '== Header' type titles, no escaping, etc.
impl Analyse for AsciiDoctor {
    fn comment_prefix(&self) -> &'static str { "//" }
    fn metadata<'a>(&self, source: &'a str) -> Metadata<'a> {
        enum M {
            Text,
            Header,
        }

        let mut state = M::Text;
        let mut prev = '\n';
        let mut walker = Walker::new(prev, source);

        let mut outline = Vec::new();
        let mut links = Vec::new();

        'main: loop {
            let start = walker.post;
            let _rest = &source[walker.post..];

            match &state {
                M::Text => {
                    while let Some((ch, curr, _)) = walker.advance() {
                        match (prev, ch) {
                            ('\n', '=') => {
                                state = M::Header;
                                continue 'main;
                            }
                            (c, 'l') if !c.is_alphabetic() && source[curr..].starts_with("link:") => {
                                walker.peek_until(|c, _| c == '[' || c.is_whitespace());
                                let pre_left_square = walker.post;
                                let uri = &source[curr + "link:".len()..pre_left_square];


                                if source[walker.post..].starts_with('[') {
                                    // AsciiDoctor sections break links
                                    // e.g. "link:url[\n\n=Ascii"
                                    let is_found = walker.peek_until(|c, i| c == ']' || source[i..].starts_with("\n\n="));
                                    if is_found {
                                        let body = &source[pre_left_square + "[".len()..walker.post];
                                        links.push((Cow::Borrowed(uri), Cow::Borrowed(body)));
                                    }
                                }

                            }
                            _ => {}
                        }

                        prev = ch;
                    }
                    break;
                }
                M::Header => {
                    walker.peek_until(|c, _| c != '=');
                    let equal_count = walker.post - start; //+ 1;
                    let rest = &source[walker.post..];

                    let line_end = rest.find('\n').unwrap_or(rest.len());
                    //walker.peek_until(|c, _| c == '\n');
                    walker.increment_post_by(line_end);
                    outline.push((equal_count as u8, Cow::Borrowed(&rest[..line_end])));
                    state = M::Text;
                }
                //_ => {}
            }
        }
        Metadata {
            outline,
            links,
        }
    }
}

////#[allow(unused_variable)]
//struct AsciiDoctor2();
//impl Analyse for AsciiDoctor2 {
//    fn metadata<'a>(&self, source: &'a str) -> Metadata<'a> {
//        // https://docs.asciidoctor.org/asciidoc/latest/blocks/
//        enum M {
//            Compound, // Can contain other blocks
//            Simple,   // Treated as cotiguous lines of paragraph text
//            Verbatim, //
//            Raw,      // Goes directly to output with no subsitutions
//            Empty,    // Contains no content
//            Table,    //
//        }
//        todo!()
//    }
//}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn works() {
        //println!("{:?}", AsciiDoctor().metadata("# hello\n== HOw are you\n==A SEcond\n")) ;
        //println!("{:?}", AsciiDoctor().metadata(FILE));
    }

    const _FILE: &str = r#"
:title: asdf

= Overwrite title

link:../hello.html[yo]

== Header 2
asdf
=== Header 3
bcde
== Header B
qwer

"#;
}
