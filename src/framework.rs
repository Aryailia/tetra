//run: cargo test -- --nocapture

use std::fmt;


#[derive(Debug)]
pub enum Source {
    Range(usize, usize),
}

#[derive(Debug)]
pub struct Token<T> {
    source: Source,
    pub me: T,
}

macro_rules! build_exact_string {
    ($buffer:ident, $( $amt:expr => $push:expr; )*) => {
        {
            let capacity =  0 $( + $amt )*;
            let mut $buffer = String::with_capacity(capacity);
            $( $push; )*
            debug_assert_eq!(capacity, $buffer.capacity());
            debug_assert_eq!(capacity, $buffer.len());
            $buffer
        }
    };
}

macro_rules! len_utf8 {
    ($char:expr => $size:literal) => {{
        debug_assert_eq!($char.len_utf8(), $size);
        $size
    }};
}

impl<T> Token<T> {
    pub fn new(me: T, source: Source) -> Self {
        Token {
            source,
            me,
        }
    }

    pub fn get_context(&self, original: &str) -> String {
        match self.source {
            // This is mimicking the formatting Rust uses for compile errors
            Source::Range(start, close) => {
                let (start, close) = if start < close {
                    (start, close)
                } else {
                    (close, start)
                };
                let offset = original.as_ptr() as usize;
                println!("get_context debug {:?}", &original[start..close]);

                // @TODO: support multiline ranges
                // @TODO: unicode width support
                let line_start = original[0..start]
                    .rfind('\n')
                    .map(|x| x + len_utf8!('\n' => 1))
                    .unwrap_or(0);
                let line_close = original[close..]
                    .find('\n')
                    .map(|x| x + close)
                    .unwrap_or(original.len());
                let line = &original[line_start..line_close];

                let row_number = original[0..start].lines().count() + 1;
                let row_number = row_number.to_string();
                let arrow_count = original[start..close].len();
                let arrow_count = std::cmp::max(arrow_count, 1);
                debug_assert!(line.find('\n').is_none());

                build_exact_string! { buffer,
                    // First line
                    len_utf8!(' ' => 1) => buffer.push(' ');
                    row_number.len() => for _ in 0..row_number.len() {
                        buffer.push(' ');
                    };
                    2 => buffer.push_str(" |");
                    len_utf8!('\n' => 1) => buffer.push('\n');


                    // Second line
                    // Indent
                    len_utf8!(' ' => 1) => buffer.push(' ');
                    row_number.len() => buffer.push_str(&row_number);
                    3 => buffer.push_str(" | ");

                    line.len() => buffer.push_str(line);
                    len_utf8!('\n' => 1) => buffer.push('\n');

                    // Third line
                    len_utf8!(' ' => 1) => buffer.push(' ');
                    row_number.len() => for _ in 0..row_number.len() {
                        buffer.push(' ');
                    };
                    3 => buffer.push_str(" | ");

                    original[line_start..start].len() => {
                        for _ in 0..original[line_start..start].len() {
                            buffer.push(' ');
                        }

                    };
                    arrow_count => for _ in 0..arrow_count {
                        buffer.push('^');
                    };
                }
            }
        }
    }
}

//#[test]
//fn hello() {
//    let file = ":title: Hello
//:bibliography:{| pandoc | cite (narrative (at_hello . )) |}
//
//== Lorem
//
//This must be a list
//
//{# because of this comment #}
//
//Meep
//
//{| if (nottrue) |}
//
//
//Come to the dark side of the moon
//
//{| endif |}
//";
//    let token = Token::new(0, Source::Range(3, 8));
//    println!("{}", token.get_context(file));
//}

impl<T: std::fmt::Debug> Token<T> {
    pub fn debug_print(&self, original: &str) {
        print!("  * {:?}", self.me);
        match self.source {
            Source::Range(start, close) => print!(" {:?}\n", &original[start..close]),
        }
    }
}
