//run: cargo test -- --nocapture

use std::mem::take;

use crate::framework::{Source, Token};

type ParseError = Token<&'static str>;
type PullParseOutput = Result<Option<Lexeme>, ParseError>;
type Lexeme = Token<LexType>;

// {_config} is a placeholder for when we pass a struct that configures
// details such as "{$ $}" should be the syntax for inline code cells
pub fn process(original: &str, _config: bool) -> Result<Vec<Lexeme>, ParseError> {
    // We add plus one for the empty string case  "" which is one lexeme long
    let mut lexemes = Vec::with_capacity(original.len() + 1);

    let mut fsm = CellFsm::new();
    let mut walker = Walker::new(original);

    while let Some(token1) = parse(&mut fsm, &mut walker)? {
        bound_push!(lexemes, token1);
    }
    //lexemes.iter().for_each(|l| println!("{:?} {:?}", l, l.to_str(original)));
    debug_assert_eq!(original, reconstruct_string(original, &lexemes));
    Ok(lexemes)
}

/******************************************************************************
 * Cell-level FSM
 ******************************************************************************/
#[derive(Clone, Debug)]
pub enum LexType {
    // Cell-level stuff
    Text,
    BlockComment,
    HereDocStart,
    HereDocClose,
    InlineStart,
    InlineClose,

    // Expression-level stuff
    Ident,
    IdentParen,
    Pipe,
    ParenStart,
    ParenClose,
    Stdin,

    ArgSeparator,
    CmdSeparator,
    Assign,

    Literal(&'static str),

    QuoteStart,
    QuoteClose,
    QuoteLiteral(&'static str),
    //Finish,
}

#[derive(Debug)]
enum CellMode {
    // Placeholder for Mode
    Transition,
    Text,    // Text block, map as-is to output
    HereDoc, // sh jargon, i.e. cell block that accepts a text block as STDIN
    Inline,  // Counterpart to 'heredoc', a regular cell block
    Comment, // Comment block
    Finish,  // Zero-span syntax, just indicates parsing is complete
}
struct CellFsm {
    mode: CellMode,
    transition_to: Option<(CellMode, Token<LexType>)>,

    heredoc: (&'static str, &'static str),
    inline: (&'static str, &'static str),
    comment: (&'static str, &'static str),

    code_fsm: CodeFsm,
}

impl CellFsm {
    fn new(/*config: Config*/) -> Self {
        Self {
            mode: CellMode::Text,
            transition_to: None,

            // The numbers are the char count
            heredoc: ("{|", "|}"),
            inline: ("{$", "$}"),
            comment: ("{#", "#}"),

            code_fsm: CodeFsm::new(),
        }
    }
}

// This essentially duplicates the behaviour of `char_indices()` but stores
// the value of the current iteration in an accessible location
struct Walker<'a> {
    original: &'a str,
    // This does not have to be peekable as we store {ch} in memory and progress
    // the "Walker" struct on-demand in `parse()`
    iter: std::str::Chars<'a>,
    ch: char,
    curr: usize,
    post: usize,
}

type WalkerStep<'a> = (char, usize, usize);
const STILL: bool = true;
const AHEAD: bool = false;

impl<'a> Walker<'a> {
    fn init_chars_iter(s: &str) -> (std::str::Chars, char, usize) {
        let mut iter = s.chars();
        let (ch, post) = iter.next().map(|c| (c, c.len_utf8())).unwrap_or((' ', 0)); // None init to anything
        (iter, ch, post)
    }

    fn new(original: &'a str) -> Self {
        // calls next on `original.chars()`
        let (iter, ch, post) = Self::init_chars_iter(original);
        Self {
            original,
            iter,
            ch,
            curr: 0, // Value should be same as `is_end()` of empty string
            post,
        }
    }

    // {is_fake_advance} is {STILL} or {AHEAD} to facilitate doing a do-while
    // loop, i.e. {STILL} on the 'do' iteration, {AHEAD} on all other iterations
    fn advance(&mut self, is_fake_advance: bool) -> Option<WalkerStep> {
        // This branch is mostly is to allow for do-while constructs
        if is_fake_advance == STILL {
            Some((self.ch, self.curr, self.post))
        } else if let Some(ch) = self.iter.next() {
            self.ch = ch;
            self.curr = self.post;
            self.post += ch.len_utf8();
            Some((ch, self.curr, self.post))
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
        self.curr += amount;
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

// General design principle is that inner loops should be smallest to optimise
// for cache hits
// i.e. We perform the most amount of repetitive skips in a small lope
// i.e. Skip whitespace in the innermost loop as oppose to an outer loop
//      Process CellMode::Text (least number of sub-branches) all in one branch
//
// Because we advance the walker iter once (in `Walker::new()`) before parsing,
// we want do-while loops. We want to move
fn parse(fsm: &mut CellFsm, walker: &mut Walker) -> PullParseOutput {
    // Each call to `parse()` only outputs one Lexeme, hence we need a
    // 'CellMode::Transition' for when there are two lexemes to output to
    // stagger them
    match fsm.mode {
        CellMode::Text => {
            let start = walker.curr;
            let mut do_while = STILL;

            while let Some((_, curr, _)) = walker.advance(do_while) {
                let current_str = &walker.original[curr..];
                let literals = [
                    ("{{|", "{|", 3),
                    ("|}}", "|}", 3),
                    ("{{$", "{$", 3),
                    ("$}}", "$}", 3),
                    ("{{#", "{#", 3),
                    ("#}}", "#}", 3),
                ];
                let (found, next_mode, transition, advance_amount) =
                    // @TODO: literals add this to configuration
                    if let Some((from, into, len)) = literals.iter().find(|x| current_str.starts_with(x.0)) {
                        debug_assert_eq!(from.len(), *len);
                        let s = Source::Range(curr, curr + *len);
                        let t = Token::new(LexType::Literal(into), s);

                        // Back to 'CellMode::Text', just using transition to
                        // push 'LexType::Literal'
                        let trans = Some((CellMode::Text, t));
                        (true, CellMode::Transition, trans, *len)

                    } else if current_str.starts_with(fsm.heredoc.0) {
                        let s = Source::Range(curr, curr + fsm.heredoc.0.len());
                        let t = Token::new(LexType::HereDocStart, s);
                        let trans = Some((CellMode::HereDoc, t));
                        (true, CellMode::Transition, trans, fsm.heredoc.0.len())
                    } else if current_str.starts_with(fsm.inline.0) {
                        let s = Source::Range(curr, curr + fsm.inline.0.len());
                        let t = Token::new(LexType::InlineStart, s);
                        let trans = Some((CellMode::Inline, t));
                        (true, CellMode::Transition, trans, fsm.inline.0.len())
                    } else if current_str.starts_with(fsm.comment.0) {
                        (true, CellMode::Comment, None, fsm.comment.0.len())
                    } else {
                        (false, CellMode::Text, None, 0)
                    };

                // If a non-'LexType::Text' lexeme found, push everything till
                // now as a Text and setup to push
                if found {
                    walker.skip(advance_amount);
                    fsm.mode = next_mode;
                    fsm.transition_to = transition;
                    let text = Token::new(LexType::Text, Source::Range(start, curr));
                    return Ok(Some(text));
                }
                do_while = AHEAD;
            }
            if walker.is_end() {
                fsm.mode = CellMode::Finish;
            }
            // Last token till the end of the file is a 'Text' lexeme.
            let text = Token::new(LexType::Text, Source::Range(start, walker.curr));
            Ok(Some(text))
        }

        CellMode::Transition => {
            let token;
            (fsm.mode, token) = take(&mut fsm.transition_to).unwrap();
            Ok(Some(token))
        }

        CellMode::Comment => {
            let start = walker.curr;
            let mut do_while = STILL;

            while let Some((_, curr, _)) = walker.advance(do_while) {
                if walker.original[curr..].starts_with(fsm.comment.1) {
                    walker.skip(fsm.comment.1.len());
                    fsm.mode = CellMode::Text;
                    let text = Token::new(LexType::BlockComment, Source::Range(start, curr));
                    return Ok(Some(text));
                }
                do_while = AHEAD;
            }

            let source = Source::Range(start, walker.post);
            Err(Token::new("Comment block no ending tag", source))
        }

        CellMode::HereDoc => {
            let (t, is_done) = lex_code_body(
                &mut fsm.code_fsm,
                walker,
                fsm.heredoc.1,
                LexType::HereDocClose,
            );
            if is_done {
                fsm.mode = CellMode::Text;
            }
            t
        }
        CellMode::Inline => {
            let (t, is_done) = lex_code_body(
                &mut fsm.code_fsm,
                walker,
                fsm.inline.1,
                LexType::InlineClose,
            );
            if is_done {
                fsm.mode = CellMode::Text;
            }
            t
        }
        //CellMode::HereDoc | CellMode::Inline => {
        //    #[allow(dead_code)]
        //    let (closer_str, closer) = match fsm.mode {
        //        CellMode::HereDoc => (fsm.heredoc.1, LexType::HereDocClose),
        //        CellMode::Inline => (fsm.inline.1, LexType::InlineClose),
        //        _ => unreachable!(),
        //    };

        //    let (t, is_done) = lex_code_body(&mut fsm.code_fsm, walker, closer_str, closer);
        //    if is_done {
        //        fsm.mode = CellMode::Text;
        //    }
        //    //if t.is_ok() {
        //    //    walker.skip(walker.ch.len_utf8() + closer_len);
        //    //    fsm.mode = CellMode::Transition;
        //    //    //fsm.transition_to = Some(());
        //    //}
        //    t
        //}
        CellMode::Finish => Ok(None),
        //_ => Ok(None),
    }
}

/******************************************************************************
 * Code-cell-level FSM
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

//

// First character must be alphabetic
fn is_invalid_second_ident_char(c: char) -> bool {
    c != '_' && (c.is_ascii_punctuation() || c.is_whitespace())
}

fn lex_code_body(
    fsm: &mut CodeFsm,
    walker: &mut Walker,
    closer_str: &str,
    closer: LexType,
) -> (PullParseOutput, bool) {
    // Eat whitespace
    walker.advance_until(|c| !c.is_whitespace());

    // Handle EOF error
    let (ch, curr, post) = if let Some(x) = walker.advance(STILL) {
        x
    } else {
        debug_assert_eq!(walker.curr, walker.original.len(), "Should be at EOF");
        let source = Source::Range(walker.curr, walker.original.len());
        let token = Token::new("Did not terminate code block", source);
        return (Err(token), false);
    };

    // Main FSM branching handling
    let (maybe_token_type, skip_amount, finished) = match (&fsm.mode, ch) {
        // Everything else
        (CodeMode::Regular, _) if walker.original[curr..].starts_with(closer_str) => {
            (closer, closer_str.len(), true)
        }
        (CodeMode::Regular, _) if ch.is_ascii_alphabetic() => {
            // First check in 'if'
            debug_assert!(
                !is_invalid_second_ident_char(ch),
                "First char of idents should also satisfy second+ char requirements"
            );
            let ident_len = walker.original[curr..]
                .find(is_invalid_second_ident_char)
                .unwrap_or(0);
            let peek_post = walker.post + ident_len;
            if &walker.original[walker.curr + ident_len..peek_post] == "(" {
                (LexType::IdentParen, ident_len + len_utf8!('(' => 1), false)
            } else {
                (LexType::Ident, ident_len, false)
            }
        }
        (CodeMode::Regular, '|') => (LexType::Pipe, len_utf8!('|' => 1), false),
        (CodeMode::Regular, '(') => (LexType::ParenStart, len_utf8!('|' => 1), false),
        (CodeMode::Regular, ')') => (LexType::ParenClose, len_utf8!('|' => 1), false),
        (CodeMode::Regular, '.') => (LexType::Stdin, len_utf8!('.' => 1), false),
        (CodeMode::Regular, ',') => (LexType::ArgSeparator, len_utf8!(',' => 1), false),
        (CodeMode::Regular, ';') => (LexType::CmdSeparator, len_utf8!(';' => 1), false),
        (CodeMode::Regular, '=') => (LexType::Assign, len_utf8!('=' => 1), false),
        (CodeMode::Regular, '"') => {
            fsm.mode = CodeMode::Quote;
            (LexType::QuoteStart, len_utf8!('"' => 1), false)
        }

        // Quotation stuff
        (CodeMode::Quote, '"') => {
            fsm.mode = CodeMode::Regular;
            (LexType::QuoteClose, 1, false)
        }
        (CodeMode::Quote, '\\') => {
            // `skip(1)` because we `advance(AHEAD)`. Effectively, we `skip(2)`
            if let Some((ch, _, _)) = walker.advance(AHEAD) {
                match ch {
                    'n' => (LexType::QuoteLiteral("\n"), len_utf8!(ch => 1), false),
                    't' => (LexType::QuoteLiteral("\t"), len_utf8!(ch => 1), false),
                    '"' => (LexType::QuoteLiteral("\""), len_utf8!(ch => 1), false),
                    ' ' | '\n' => (LexType::QuoteLiteral(""), len_utf8!(ch => 1), false),
                    _ => {
                        let source = Source::Range(curr, post);
                        let token = Token::new("Missing closing quotation mark", source);
                        return (Err(token), false);
                    }
                }
            } else {
                let source = Source::Range(curr, walker.original.len());
                let token = Token::new("Missing closing quotation mark", source);
                return (Err(token), false);
            }
        }
        (CodeMode::Quote, _) => {
            let mut do_while = STILL;
            while let Some((ch, _, _)) = walker.advance(do_while) {
                if ch == '"' || ch == '\\' {
                    break;
                }
                do_while = AHEAD;
            }

            if walker.is_end() {
                let source = Source::Range(curr, post);
                let token = Token::new("Missing closing quotation mark", source);
                return (Err(token), false);
            }

            // `skip(0)` because we already advanced {walker}
            debug_assert!(walker.ch == '"' || walker.ch == '\\');
            (LexType::Text, 0, false)
        }

        //(CodeMode::Quote, '\"') => {
        //    walker.advance(AHEAD);
        //    LexType::QuoteStart
        //}
        _ => {
            let source = Source::Range(curr, post);
            println!("CodeMode::{:?}", fsm.mode);
            return (Err(Token::new("lexer.rs: Invalid Syntax", source)), false);
        }
    };

    walker.skip(skip_amount);
    let source = Source::Range(curr, walker.curr);
    let token = Token::new(maybe_token_type, source);
    (Ok(Some(token)), finished)
}

/******************************************************************************
 * Functions for use in testing
 ******************************************************************************/
// Remakes the {original} from {lexemes}
fn reconstruct_string(original: &str, lexemes: &[Lexeme]) -> String {
    // @TODO: Input config as an argument
    struct Config {
        heredoc: (&'static str, &'static str),
        inline: (&'static str, &'static str),
        comment: (&'static str, &'static str),
    }
    let config = Config {
        heredoc: ("{|", "|}"),
        inline: ("{$", "$}"),
        comment: ("{#", "#}"),
    };
    let mut buffer = String::with_capacity(original.len());
    let mut mode = CellMode::Text;

    // Because the lexing process completely ignores whitespace, we reconstruct
    // {original} by changing each lexeme into its string equivalent and then
    // checking {original} to see if we need to add any whitespace.
    for token in lexemes {
        let text = match token.source {
            Source::Range(start, close) => &original[start..close],
        };

        // Whitespace is only deleted in code cells
        // Add whitespace based on {original} and our current position
        match mode {
            CellMode::HereDoc | CellMode::Inline => {
                let len = buffer.len();
                let remaining = &original[len..];
                let whitespace_len = remaining.find(|c: char| !c.is_whitespace()).unwrap_or(0);

                //println!("{:?} {:?}", whitespace_len, text);

                buffer.push_str(&original[len..len + whitespace_len]);
                assert_eq!(
                    &original[0..len + whitespace_len],
                    buffer,
                    "\n\nAdded {:?}\n",
                    &original[len..len + whitespace_len],
                );
            }
            _ => {}
        }

        macro_rules! push_check {
            ($buffer:ident $char:literal if $text:ident == $str:literal ) => {{
                assert_eq!($str, $text);
                $buffer.push($char);
            }};
        }

        // Convert each lexeme to its string equivalent and push onto the buffer
        match token.me {
            LexType::Text => buffer.push_str(text),
            LexType::BlockComment => {
                buffer.push_str(config.comment.0);
                buffer.push_str(text);
                buffer.push_str(config.comment.1);
            }
            LexType::HereDocStart => {
                mode = CellMode::HereDoc;
                buffer.push_str(config.heredoc.0);
            }
            LexType::InlineStart => {
                mode = CellMode::Inline;
                buffer.push_str(config.inline.0);
            }
            LexType::HereDocClose => {
                mode = CellMode::Text;
                buffer.push_str(config.heredoc.1);
            }
            LexType::InlineClose => {
                mode = CellMode::Text;
                buffer.push_str(config.inline.1);
            }

            LexType::ArgSeparator => push_check!(buffer ',' if text == ","),
            LexType::CmdSeparator => push_check!(buffer ';' if text == ";"),
            LexType::Assign => push_check!(buffer '=' if text == "="),

            LexType::Ident => {
                assert!(text.find(is_invalid_second_ident_char).is_none());
                buffer.push_str(text);
            }
            LexType::IdentParen => {
                let penultimate_post = text.len() - len_utf8!('(' => 1);
                assert!(text[..penultimate_post]
                    .find(is_invalid_second_ident_char)
                    .is_none());
                assert_eq!("(", &text[penultimate_post..]);
                buffer.push_str(text);
            }
            LexType::Pipe => push_check!(buffer '|' if text == "|"),
            LexType::ParenStart => push_check!(buffer '(' if text == "("),
            LexType::ParenClose => push_check!(buffer ')' if text == ")"),
            LexType::Stdin => push_check!(buffer '.' if text == "."),

            LexType::Literal("{|") => buffer.push_str("{{|"),
            LexType::Literal("|}") => buffer.push_str("|}}"),
            LexType::Literal("{$") => buffer.push_str("{{$"),
            LexType::Literal("$}") => buffer.push_str("$}}"),
            LexType::Literal("{#") => buffer.push_str("{{#"),
            LexType::Literal("#}") => buffer.push_str("#}}"),
            LexType::Literal(_) => unreachable!(),

            LexType::QuoteStart | LexType::QuoteClose => push_check!(buffer '"' if text == "\""),
            LexType::QuoteLiteral(_) => buffer.push_str(text),
        }
    }

    assert_eq!(original.len(), buffer.capacity());
    assert_eq!(original.len(), buffer.len());
    buffer
}
