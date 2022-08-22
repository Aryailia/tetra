//run: cargo test -- --nocapture

use pulldown_cmark::{CowStr, Event, Parser, Tag};
use std::borrow::Cow;
use std::collections::HashMap;
use std::mem;

use crate::{Analyse, Metadata};

pub struct CommonMark();

// Currently pulldown_cmark does not support frontmatter
// See #580: https://github.com/raphlinus/pulldown-cmark/issues/580
//
// So we do it ourselves
impl Analyse for CommonMark {
    fn comment_prefix(&self) -> &'static str {
        "<!--"
    }

    fn metadata<'a>(&self, source: &'a str) -> Metadata<'a> {
        let frontmatter = source.strip_prefix("---\n").and_then(|post_dashes| {
            // @TODO: simplify this into one loop? Might be less idomatic
            if let Some(end) = post_dashes.find("\n---\n") {
                Some(&post_dashes[..end])
            } else {
                source.strip_suffix("\n---")
            }
        }).unwrap_or("");

        let mut attributes = HashMap::new();
        for line in frontmatter.lines() {
            if let Some(colon_index) = line.find(':') {
                let (key, val) = line.split_at(colon_index);
                attributes.insert(key, val[":".len()..].trim());
            }
        }

        ////////////////////////////////////////////////////////////////////////
        let mut is_header = false;
        let mut is_link = false;

        //let mut options = Options::empty();
        //let mut parser = Parser::new_ext(source, options);

        let mut build_link = (Cow::Borrowed(""), Cow::Borrowed(""));
        let mut build_header = (0, Cow::Borrowed(""));
        let mut outline = Vec::new();
        let mut links = Vec::new();
        for event in Parser::new(&source[frontmatter.len()..]) {
            let mut event_text = None;
            match event {
                Event::Start(Tag::Heading(heading_level, _, _)) => {
                    debug_assert!(!is_header);
                    build_header.0 = heading_level as u8;
                    is_header = true;
                }
                Event::End(Tag::Heading(_, _, _)) => {
                    debug_assert!(is_header);
                    is_header = false;
                    let default = (0, Cow::Borrowed(""));
                    outline.push(mem::replace(&mut build_header, default));
                }

                Event::Start(Tag::Link(_, url, _)) => {
                    debug_assert!(!is_link);
                    is_link = true;
                    build_link.0 = cowstr_to_cow(url);
                }
                Event::End(Tag::Link(_, _, _)) => {
                    debug_assert!(is_link);
                    is_link = false;
                    let default = (Cow::Borrowed(""), Cow::Borrowed(""));
                    links.push(mem::replace(&mut build_link, default));
                }

                Event::Text(cowstr) => event_text = Some(cowstr),
                _ => { /* println!("{:?}", event); */ }
            };

            if let Some(text) = event_text {
                // There might be a link in the header, so we have to `.clone()`
                if is_header {
                    match &mut build_header.1 {
                        Cow::Borrowed("") => build_header.1 = cowstr_to_cow(text.clone()),
                        Cow::Borrowed(s) => build_header.1 = Cow::Owned([*s, &text].join("")),
                        Cow::Owned(s) => s.push_str(&text),
                    }
                }
                if is_link {
                    match &mut build_link.1 {
                        Cow::Borrowed("") => build_link.1 = cowstr_to_cow(text),
                        Cow::Borrowed(s) => build_link.1 = Cow::Owned([*s, &text].join("")),
                        Cow::Owned(s) => s.push_str(&text),
                    }
                }
            }
        }
        Metadata {
            outline,
            links,
            attributes,
        }
    }
}

fn cowstr_to_cow(custom: CowStr) -> Cow<str> {
    match &custom {
        CowStr::Boxed(_) | CowStr::Inlined(_) => Cow::Owned(custom.to_string()),
        CowStr::Borrowed(s) => Cow::Borrowed(s),
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn works() {
        println!("{:?}", CommonMark().metadata(_FILE));
        //println!("{:?}", AsciiDoctor().outline("# hello\n== HOw are you\n==A SEcond\n")) ;
    }

    const _FILE: &str = r#"---
test: yo
---
# Beautiful

lorem ipsum

## My Name is *bob*

[hello the **black** cat](https://www.gnu.org)

"#;
}
