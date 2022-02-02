//run: cargo test -- --nocapture

use crate::framework::{Token, Source};
use std::mem::take;


#[derive(Debug)]
pub enum LexType {
    // Cell-level stuff
    Text,
    BlockComment,
    HereDoc,
    Inline,
    CodeClose,

    // Expression-level stuff
    Ident,
    Pipe,
    ParenStart,
    ParenClose,
    Stdin,

    QuoteStart,
    QuoteClose,
    Quoted,

    Finish,
}

enum CellMode {
    // Placeholder for Mode
    Transition,
    Text,       // Text block, map as-is to output
    HereDoc,    // sh jargon, i.e. cell block that accepts a text block as STDIN
    Inline,     // Counterpart to 'heredoc', a regular cell block
    Comment,    // Comment block
    Finish,     // Zero-span syntax, just indicates parsing is complete
}

type Lexeme = Token<LexType>;

//
pub fn process(original: &str, config: bool) -> PullParseOutput {
    let mut lexemes = Vec::with_capacity(original.len());

    let mut fsm = CellFsm::new();
    let mut walker = Walker::new(original);

    //println!("{:?}", original);
    //println!("{:?}", walker.advance(true).unwrap().0);
    //println!("{:?}", walker.advance(false).unwrap().0);
    //walker.skip(2);
    //println!("{:?}", walker.advance(true).unwrap().0);
    //println!("{:?}", walker.advance(false).unwrap().0);
    //println!("{:?}", walker.advance(false).unwrap().0);
    //println!("{:?}", walker.advance(false));

    while let Some(token1) = parse(&mut fsm, &mut walker)? {
        lexemes.push(token1);
        //maybe_token2.map(|t| lexemes.push(t));
    }
    //lexemes.push(Token::new(LexType::Text, Source::Range(0, 0)));
    for l in lexemes {
        l.debug_print(original);
    }
    //println!("{:?}", original);
    //println!("{:?}", lexemes.len());

    Err(Token::new("DEV error", Source::Range(0, 0)))
}


struct CellFsm {
    mode: CellMode,
    transition_to: Option<(CellMode, Token<LexType>)>,

    heredoc: (&'static str, &'static str),
    heredoc_len: (usize, usize),
    inline: (&'static str, &'static str),
    inline_len: (usize, usize),
    comment: (&'static str, &'static str),
    comment_len: (usize, usize),

    code_fsm: CodeFsm,
}

impl CellFsm {
    fn new(/*config: Config*/) -> Self {
        Self {
            mode: CellMode::Text,
            transition_to: None,

            // The numbers are the char count
            heredoc: ("{|", "|}"),
            heredoc_len: (2, 2),
            inline: ("{{", "}}"),
            inline_len: (2, 2),
            comment: ("{#", "#}"),
            comment_len: (2, 2),

            code_fsm: CodeFsm::new(),
        }
    }
}

struct Walker<'a> {
    original: &'a str,
    // This does not have to be peekable as we store {ch} in memory and progress
    // the "Walker" struct on-demand in `parse()`
    iter: std::str::Chars<'a>,
    ch: char,
    rest: &'a str,
    curr: usize,
    post: usize,
}

type WalkerStep<'a> = (char, usize, usize, &'a str);
impl<'a> Walker<'a> {
    fn init_chars_iter(s: &str) -> (std::str::Chars, char, usize) {
        let mut iter = s.chars();
        let (ch, post) = iter
            .next()
            .map(|c| (c, c.len_utf8()))
            .unwrap_or((' ', 0)); // None init to anything
        (iter, ch, post)
    }

    fn new(original: &'a str) -> Self {
        // calls next on `original.chars()`
        let (iter, ch, post) = Self::init_chars_iter(original);
        let rest = iter.as_str();
        Self {
            original,
            iter,
            rest,
            ch,
            curr: 0, // Value should be same as `is_end()` of empty string
            post,
        }
    }



    fn advance(&mut self, is_fake_advance: bool) -> Option<WalkerStep> {
        // This branch is mostly is to allow for do-while constructs
        if is_fake_advance {
            Some((self.ch, self.curr, self.post, self.rest))
        } else if let Some(ch) = self.iter.next() {
            self.ch = ch;
            self.rest = self.iter.as_str();
            self.curr = self.post;
            self.post += ch.len_utf8();
            Some((ch, self.curr, self.post, self.rest))
        } else {
            self.curr = self.original.len();
            None
        }
        //self.post = curr + ch.len_utf8();
    }

    // @TODO: eliminate boundary check of chars().next()
    fn skip(&mut self, amount: usize) {
        // Fast path
        let substr = &self.original[self.curr + amount..];
        let len_utf8;
        (self.iter, self.ch, len_utf8) = Self::init_chars_iter(substr);
        self.rest = self.iter.as_str();
        self.curr = self.curr + amount;
        self.post = self.curr + len_utf8;

        //println!("skip {:?}", &self.original[self.post + amount..]);
    }

    fn advance_until<P: Fn(char) -> bool>(&mut self, predicate: P) {
        //println!("{:?}", &self.original[self.curr..]);
        let amount = self.original[self.curr..].find(predicate).unwrap_or(0);
        self.skip(amount);
    }

    #[inline]
    fn is_end(&self) -> bool {
        self.curr == self.original.len()
    }
}

#[test]
fn walk_empty_string() {
    // `is_end()` of empty string should evaluate to 0
    let walker = Walker::new("");
    debug_assert!(walker.is_end());
}

type PullParseOutput = Result<Option<Lexeme>, Token<&'static str>>;

// General design principle is that inner loops should be smallest to optimise
// for cache hits
// i.e. We perform the most amount of repetitive skips in a small lope
// i.e. Skip whitespace in the innermost loop as oppose to an outer loop
//      Process CellMode::Text (least number of sub-branches) all in one branch
//
// Because we advance the walker iter once (in `Walker::new()`) before parsing,
// we want do-while loops. We want to move
fn parse(fsm: &mut CellFsm, walker: &mut Walker) -> PullParseOutput {
    match fsm.mode {
        CellMode::Text => {
            let start = walker.curr;
            let mut do_while = true;

            while let Some((ch, curr, post, rest)) = walker.advance(do_while) {
                let (found, next_mode, transition, advance_amount) =
                    if rest.starts_with(fsm.heredoc.0) {
                        let s = Source::Range(post, post + fsm.heredoc_len.0);
                        let t = Token::new(LexType::HereDoc, s);
                        let trans = Some((CellMode::HereDoc, t));
                        (true, CellMode::Transition, trans, fsm.heredoc_len.0)
                    } else if rest.starts_with(fsm.inline.0) {
                        let s = Source::Range(post, post + fsm.inline_len.0);
                        let t = Token::new(LexType::Inline, s);
                        let trans = Some((CellMode::Text, t));
                        (true, CellMode::Transition, trans, fsm.inline_len.0)
                    } else if rest.starts_with(fsm.comment.0) {
                        (true, CellMode::Comment, None, fsm.comment_len.0)
                    } else {
                        (false, CellMode::Text, None, 0)
                    };


                if found {
                    walker.skip(ch.len_utf8() + advance_amount);
                    fsm.mode = next_mode;
                    fsm.transition_to = transition;
                    let text = Token::new(LexType::Text, Source::Range(start, post));
                    return Ok(Some(text));
                }
                do_while = false;
            }
            if walker.is_end() {
                fsm.mode = CellMode::Finish;
            }
            let text = Token::new(LexType::Text, Source::Range(start, walker.post));
            Ok(Some(text))
        }
        CellMode::Transition => {
            let token;
            (fsm.mode, token) = take(&mut fsm.transition_to).unwrap();
            Ok(Some(token))
        }

        CellMode::Comment => {
            let start = walker.curr;
            let mut do_while = true;

            while let Some((ch, curr, post, rest)) = walker.advance(do_while) {
                if rest.starts_with(fsm.comment.1) {
                    walker.skip(ch.len_utf8() + fsm.comment_len.1);
                    fsm.mode = CellMode::Text;
                    let text = Token::new(LexType::BlockComment, Source::Range(start, post));
                    return Ok(Some(text))
                }
                do_while = false;
            }

            let source = Source::Range(start, walker.post);
            Err(Token::new("Comment block no ending tag", source))
        }

        CellMode::HereDoc | CellMode::Inline => {
            #[allow(dead_code)]
            let (closer_str, closer_len) = match fsm.mode {
                CellMode::HereDoc => (fsm.heredoc.1, fsm.heredoc_len.1),
                CellMode::Inline => (fsm.inline.1, fsm.inline_len.1),
                _ => unreachable!(),
            };

            let (t, is_done) = lex_code_body(&mut fsm.code_fsm, walker, closer_str);
            if is_done {
                fsm.mode = CellMode::Text;
            }
            //if t.is_ok() {
            //    walker.skip(walker.ch.len_utf8() + closer_len);
            //    fsm.mode = CellMode::Transition;
            //    //fsm.transition_to = Some(());
            //}
            t
        }
        CellMode::Finish => Ok(None),
        //_ => Ok(None),
    }
}

/******************************************************************************
 * 'PullParser' impl for 'StatementFsm'
 ******************************************************************************/
// This
struct CodeFsm {
    mode: CodeMode,
}

impl CodeFsm {
    fn new() -> Self {
        Self {
            mode: CodeMode::Regular,
        }
    }
}

#[derive(Debug)]
enum CodeMode {
    Regular,
    Quote,
}


const STILL: bool = true;
const AHEAD: bool = false;
//

// First character must be alphabetic
fn is_invalid_second_ident_char(c: char) -> bool {
    c != '_' && (c.is_ascii_punctuation() || c.is_whitespace())
}

fn lex_code_body(
    fsm: &mut CodeFsm, walker: &mut Walker, closer: &str
) -> (PullParseOutput, bool) {

    walker.advance_until(|c| !c.is_whitespace());

    if let Some((ch, curr, post, rest)) = walker.advance(true) {
        let (maybe_token_type, finished) = match (&fsm.mode, ch) {
            (CodeMode::Quote, '"') => {
                walker.advance(AHEAD);
                fsm.mode = CodeMode::Regular;
                (LexType::QuoteClose, false)
            }
            (CodeMode::Quote, _) => {
                (LexType::Ident, false)
            }


            (CodeMode::Regular, _) if walker.original[curr..].starts_with(closer) => {
                walker.skip(closer.len());
                (LexType::CodeClose, true)
            }
            (CodeMode::Regular, _) if ch.is_ascii_alphabetic() => {
                // First check in 'if'
                debug_assert!(
                    !is_invalid_second_ident_char(ch),
                    "First char of idents should also satisfy second+ char requirements"
                );
                walker.advance_until(is_invalid_second_ident_char);
                (LexType::Ident, false)
            }
            (CodeMode::Regular, '|') => {
                walker.advance(AHEAD);
                (LexType::Pipe, false)
            }
            (CodeMode::Regular, '(') => {
                walker.advance(AHEAD);
                (LexType::ParenStart, false)
            }
            (CodeMode::Regular, ')') => {
                walker.advance(AHEAD);
                (LexType::ParenClose, false)
            }
            (CodeMode::Regular, '.') => {
                walker.advance(AHEAD);
                (LexType::Stdin, false)
            }
            //(CodeMode::Regular, '\"') => {
            //    walker.advance(AHEAD);
            //    LexType::QuoteStart
            //}
            _ => {
                let source = Source::Range(curr, post);
                return (Err(Token::new("Invalid Syntax", source)), false)
            }
        };

        let source = Source::Range(curr, walker.curr);
        let token = Token::new(maybe_token_type, source);
        (Ok(Some(token)), finished)
    } else {
        debug_assert_eq!(walker.curr, walker.original.len(), "Should be at EOF");
        let source = Source::Range(walker.curr, walker.original.len());
        let token = Token::new("Did not terminate code block", source);
        (Err(token), false)
    }
    //let start = walker.curr;
    //let mut do_while = true;

    //while let Some((ch, curr, post, rest)) = walker.advance(do_while) {
    //    match ch {
    //        _ if rest.starts_with(closer_str) => {
    //        }
    //    }
    //    if rest.starts_with(closer_str) {
    //        //walker.skip(ch.len_utf8() + fsm.comment_len.1);
    //        //fsm.mode = CellMode::Text;
    //        //let text = Token::new(LexType::BlockComment, Source::Range(start, post));
    //        //return Ok(Some((text, None)))
    //    } else {
    //        lex_code_body(&mut codefsm, (ch, curr, post, rest));
    //    }
    //    do_while = false;
    //}
    ////match {
    ////}

    //Err("Code block with no ending tag".to_string())
}

