//run: cargo test -- --nocapture

macro_rules! len_utf8 {
    ($char:expr => $size:literal) => {{
        debug_assert_eq!($char.len_utf8(), $size);
        $size
    }};
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

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Source {
    Range(usize, usize),
}

impl Source {
    pub fn to_str<'a>(&self, original: &'a str) -> &'a str {
        match self {
            Source::Range(start, close) => &original[*start..*close],
        }
    }

    pub fn get_context(&self, original: &str) -> String {
        match self {
            // This is mimicking the formatting Rust uses for compile errors
            Source::Range(start, close) => {
                let (start, close) = if *start < *close {
                    (*start, *close)
                } else {
                    (*close, *start)
                };
                //let offset = original.as_ptr() as usize;

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

                if line.find('\n').is_some() {
                    todo!("Multiline string error\n    {:?}\n", line.lines().next());
                }
                //debug_assert!(line.find('\n').is_none());

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

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Token<T> {
    pub source: Source,
    pub me: T,
}

impl<T> Token<T> {
    pub fn new(me: T, source: Source) -> Self {
        Token { source, me }
    }

    pub fn remap<U>(&self, new: U) -> Token<U> {
        Token {
            me: new,
            source: self.source.clone(),
        }
    }

    pub fn to_str<'a>(&self, original: &'a str) -> &'a str {
        self.source.to_str(original)
    }

    pub fn get_context(&self, original: &str) -> String {
        self.source.get_context(original)
    }
}

//pub trait BoundPush<T> {
//    fn bound_push(&mut self, entry: T) {
//        debug_assert!(self.len() < self.capacity());
//        self.push(entry);
//    }
//
//    fn push(&mut self, entry: T);
//    fn len(&self) -> usize;
//    fn capacity(&self) -> usize;
//}
//
//impl<T> BoundPush<T> for Vec<T> {
//    fn push(&mut self, entry: T) { self.push(entry); }
//    fn len(&self) -> usize { self.len() }
//    fn capacity(&self) -> usize { self.capacity() }
//}

// Use a macro over the traits so that we still get compiler messages
// from the correct source code line
macro_rules! bound_push {
    ($vec:expr, $entry:expr) => {{
        debug_assert!($vec.len() < $vec.capacity(), "Pushing will grow the array {} -> {}", $vec.len(), $vec.capacity());
        $vec.push($entry);
    }};
}

//#[test]
//#[should_panic]
//fn checking_bound_push() {
//    let mut a = Vec::with_capacity(2);
//    bound_push!(a, 0);
//    bound_push!(a, 1);
//    bound_push!(a, 2);
//    //a.bound_push(0);
//    //a.bound_push(1);
//    //a.bound_push(2);
//}

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
            Source::Range(start, close) => println!(" {:?}", &original[start..close]),
        }
    }
}
