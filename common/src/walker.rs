//run: cargo test -- --nocapture

// This essentially duplicates the behaviour of `char_indices()` but stores
// the value of the current iteration in an accessible location
// the value of the current iteration in an accessible location
pub struct Walker<'a> {
    pub original: &'a str,
    // This does not have to be peekable as we store {ch} in memory and progress
    // the "Walker" struct on-demand in `parse()`
    iter: std::iter::Peekable<std::str::Chars<'a>>,
    ch: char,
    curr: usize,
    pub post: usize,
}

type WalkerStep<'a> = (char, usize, usize);

impl<'a> Walker<'a> {
    pub fn new(init: char, original: &'a str) -> Self {
        // calls next on `original.chars()`
        //let (iter, ch, post) = Self::init_chars_iter(original);
        Self {
            original,
            iter: original.chars().peekable(),
            ch: init,
            curr: 0, // Value should be same as `is_end()` of empty string
            post: 0,
        }
    }

    pub fn current(&self) -> WalkerStep {
        (self.ch, self.curr, self.post)
    }

    pub fn advance(&mut self) -> Option<WalkerStep> {
        self.curr = self.post;
        self.ch = self.iter.next()?;
        self.post += self.ch.len_utf8();
        Some((self.ch, self.curr, self.post))
    }

    // You should call {advance()}, otherwise {self.ch} and {self.curr} are
    // set incorrectly
    pub fn increment_post_by(&mut self, amount: usize) {
        // Fast path because it does not set
        self.post += amount;
        debug_assert!(self.original.is_char_boundary(self.post));
        self.iter = self.original[self.post..].chars().peekable();

        //self.ch = ch;
        //self.curr = self.original.ciel_char_boundary(self.post.saturating_sub(1));

        //println!("skip {:?}", &self.original[self.post + amount..]);
    }

    // You should call {advance()}, otherwise {self.ch} and {self.curr} are
    // set incorrectly
    pub fn peek_until<P: Fn(char, usize) -> bool>(&mut self, predicate: P) -> bool {
        //println!("{:?}", &self.original[self.post..]);
        while let Some(ch) = self.iter.peek() {
            if predicate(*ch, self.post) {
                return true;
            } else {
                self.advance();
            }
        }
        false
    }

    pub fn peek(&mut self) -> Option<&char> {
        self.iter.peek()
    }
}

#[test]
fn walk_empty_string() {
    // `is_end()` of empty string should evaluate to 0
    let init = '\n';
    let mut walker = Walker::new(init, "");
    assert_eq!(None, walker.advance());
    assert_eq!(None, walker.advance());
    assert_eq!((init, 0, 0), walker.current());
}

#[test]
fn walk_str() {
    let a = "a👩b🔬あc👩‍🔬d";
    let init = '\n';
    let mut walker = Walker::new(init, a);
    assert_eq!((init, 0, 0), walker.current());
    assert_eq!(Some(('a', 0, 1)), walker.advance());
    assert_eq!(Some(('\u{1f469}', 1, 5)), walker.advance()); // Woman emoji
    assert_eq!(Some(('b', 5, 6)), walker.advance()); // a
    assert_eq!(Some(('\u{1f52c}', 6, 10)), walker.advance()); // Microscope emoji
    assert_eq!(Some(('\u{3042}', 10, 13)), walker.advance()); // hiragana 'a'
    assert_eq!(Some(('c', 13, 14)), walker.advance()); // b

    assert_eq!(Some(('\u{1f469}', 14, 18)), walker.advance()); // Woman emoji
    assert_eq!(Some(('\u{200d}', 18, 21)), walker.advance()); // Zero-width join
    assert_eq!(Some(('\u{1f52c}', 21, 25)), walker.advance()); // Microscope emoji
    assert_eq!(Some(('d', 25, 26)), walker.advance()); // Microscope emoji
    assert_eq!(None, walker.advance());
    assert_eq!(None, walker.advance());
    assert_eq!(('d', 26, 26), walker.current());

    let mut walker = Walker::new(init, a);
    walker.increment_post_by(5);
    assert_eq!(Some(('b', 5, 6)), walker.advance());
}


#[test]
fn advance_until() {
    let original = "a   b    c";
    let mut walker = Walker::new('\n', original);
    walker.peek_until(|c, curr| {
        assert_eq!(original, &walker.original[curr..]);
        !c.is_whitespace()
    });
    walker.peek_until(|c, _| !c.is_whitespace());
    assert_eq!(Some(('a', 0, 1)), walker.advance());
    walker.peek_until(|c, _| !c.is_whitespace());
    walker.peek_until(|c, curr| {
        assert_eq!("b    c", &walker.original[curr..]);
        !c.is_whitespace()
    });
    assert_eq!(Some(('b', 4, 5)), walker.advance());
    walker.peek_until(|c, _| !c.is_whitespace());
    walker.peek_until(|c, _| !c.is_whitespace());
    walker.peek_until(|c, _| !c.is_whitespace());
    assert_eq!(Some(('c', 9, 10)), walker.advance());
}
