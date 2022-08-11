//run: cargo test -- --nocapture

use std::fmt::Write as _;

use super::Metadata;

impl<'a> Metadata<'a> {
    // No 'Syn' dependency so no 'Serde'.
    pub fn to_json(&self) -> String {
        let mut buffer = String::new();
        buffer.push_str("{\"outline\":[");

        let mut iter = self.outline.iter().peekable();
        while let Some((level, body)) = iter.next() {
            write!(buffer, "[{},", level).unwrap();
            json_push_str(&mut buffer, body);
            buffer.push(']');
            if iter.peek().is_some() {
                buffer.push(',');
            }
        }

        buffer.push_str("],\"links\":[");

        let mut iter = self.links.iter().peekable();
        while let Some((uri, body)) = iter.next() {
            buffer.push('[');
            json_push_str(&mut buffer, uri);
            buffer.push(',');
            json_push_str(&mut buffer, body);
            buffer.push(']');
            if iter.peek().is_some() {
                buffer.push(',');
            }
        }

        buffer.push_str("],\"attributes\":{");
        let mut iter = self.attributes.iter().peekable();
        while let Some((key, val)) = iter.next() {
            json_push_str(&mut buffer, key);
            buffer.push(':');
            json_push_str(&mut buffer, val);
            if iter.peek().is_some() {
                buffer.push(',');
            }
        }
        buffer.push_str("}}");
        buffer
    }
}

fn json_push_str(buffer: &mut String, to_push: &str) {
    buffer.push('"');

    // @TODO: This would actually be a good place to use SIMD
    for c in to_push.chars() {
        match c {
            '\\' => buffer.push_str("\\\\"),
            '\u{0008}' => buffer.push_str("\\b"),
            '\u{000c}' => buffer.push_str("\\f"),
            '\n' => buffer.push_str("\\n"),
            '\r' => buffer.push_str("\\r"),
            '\t' => buffer.push_str("\\t"),
            '"' => buffer.push_str("\\\""),
            c if c.is_control() => write!(buffer, "\\u{:04x}", c as u32).unwrap(),
            c => buffer.push(c),
        }
    }
    buffer.push('"');
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use crate::{Analyse, FileType};

    use std::io::Write;
    use std::process::{Command, Stdio};

    #[test]
    fn it_works() {
        assert_eq!("//", FileType::from("adoc").unwrap().comment_prefix());
        let file = std::fs::read_to_string("../readme-source.md").unwrap();
        //println!("{}", FileType::CommonMark.metadata(&file).to_json());
        assert_valid_json(&FileType::CommonMark.metadata(&file).to_json());
        //FileType::AsciiDoctor.metadata("= Yo");
    }

    // Check is valid by running through jq
    fn assert_valid_json(json_str: &str) {
        let child = Command::new("jq")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Could not run jq");
        write!(child.stdin.as_ref().unwrap(), "{}", json_str)
            .expect("Could not write to jq's STIDN");
        let output = child.wait_with_output().expect("jq failed to run");
        if !output.status.success() {
            panic!(
                "{}\n=== Source JSON ===\n{}",
                std::str::from_utf8(&output.stderr).expect("Did not return utf8 error"),
                json_str,
            );
        }
    }
}
