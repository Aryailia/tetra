//run: cargo test -- --nocapture

use std::mem::replace;

use crate::framework::{Source, Token};
use common::Walker;

type ParseError = Token<&'static str>;
type PullResult<T> = Result<T, ParseError>;
type Lexeme = Token<LexType>;

struct Config<'a> {
    heredoc: (&'static str, &'static str),
    inline: (&'static str, &'static str),
    comment: (&'static str, &'static str),
    literals: &'a [(&'static str, &'static str)],
}
// {_config} is a placeholder for when we pass a struct that configures
// details such as "{$ $}" should be the syntax for inline code cells
pub fn process(original: &str, _config: bool) -> PullResult<Vec<Lexeme>> {
    // We add plus one for the empty string case  "" which is one lexeme long
    let mut lexemes = Vec::with_capacity(original.len() + 1);

    let mut fsm = CellFsm::new();
    let mut walker = Walker::new('\n', original); // Don't use init = '\n'

    let config = Config {
        heredoc: ("{|", "|}"),
        inline: ("{$", "$}"),
        comment: ("{#", "#}"),
        literals: &[
            ("{{|", "{|"),
            ("|}}", "|}"),
            ("{{$", "{$"),
            ("$}}", "$}"),
            ("{{#", "{#"),
            ("#}}", "#}"),
        ],
    };

    while let Some(token1) = parse(&mut fsm, &mut walker, &config)? {
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

    KeyValSeparator,
    ArgSeparator,
    StmtSeparator,
    Assign,

    Literal(&'static str),

    QuoteStart,
    QuoteClose,
    QuoteLiteral(&'static str),
    //Finish,
}

#[derive(Debug)]
enum CellMode {
    // 'CellMode::Transition' queues a second push
    Transition(Option<Lexeme>), // Queueing 'None' ends the loop in 'process()'
    Text,        // Text block, map as-is to output
    HereDoc,     // sh jargon, i.e. cell block that accepts a text block as STDIN
    Inline,      // Counterpart to 'heredoc', a regular cell block
    Comment,     // Comment block
}


struct CellFsm {
    mode: CellMode,
    transition_to: CellMode,

    code_mode: CodeMode,
}

impl CellFsm {
    fn new() -> Self {
        Self {
            mode: CellMode::Text,
            transition_to: CellMode::Text, // Default does not matter

            code_mode: CodeMode::Regular,
        }
    }
}

// General design principle is that inner loops should be smallest to optimise
// for cache hits
// i.e. We perform the most amount of repetitive skips in a small lope
// i.e. Skip whitespace in the innermost loop as oppose to an outer loop
//      Process CellMode::Text (least number of sub-branches) all in one branch
//
// Because we advance the walker iter once (in `Walker::new()`) before parsing,
// we want do-while loops. We want to move
fn parse(fsm: &mut CellFsm, walker: &mut Walker, cfg: &Config) -> PullResult<Option<Lexeme>> {
    // Each call to `parse()` only outputs one Lexeme, hence we need a
    // 'CellMode::Transition' for when there are two lexemes to output to
    // stagger them
    match &mut fsm.mode {
        CellMode::Text => {
            // {walker.post} is the only thing that is safe since we might have
            // ended from a different match branch with 'increment_post_by()'.
            let start = walker.post;
            let mut post = walker.post;

            loop {
                let current_str = &walker.original[walker.post..];

                let (found, next_mode, transition, skip_amt) =
                    if let Some((from, into)) = cfg.literals.iter().find(|x| current_str.starts_with(x.0)) {
                        let s = Source::Range(post, post + from.len());
                        let t = Some(Token::new(LexType::Literal(into), s));
                        (true, CellMode::Transition(t), CellMode::Text, from.len())

                    } else if current_str.starts_with(cfg.heredoc.0) {
                        let s = Source::Range(post, post + cfg.heredoc.0.len());
                        let t = Some(Token::new(LexType::HereDocStart, s));
                        (true, CellMode::Transition(t), CellMode::HereDoc, cfg.heredoc.0.len())

                    } else if current_str.starts_with(cfg.inline.0) {
                        let s = Source::Range(post, post + cfg.inline.0.len());
                        let t = Some(Token::new(LexType::InlineStart, s));
                        (true, CellMode::Transition(t), CellMode::Inline, cfg.inline.0.len())

                    } else if current_str.starts_with(cfg.comment.0) {
                        (true, CellMode::Comment, CellMode::Comment, cfg.comment.0.len())
                    } else {
                        (false, CellMode::Text, CellMode::Text, 0)
                    };

                // These happen for all if-branches except the found-nothing branch
                if found {
                    fsm.mode = next_mode;
                    fsm.transition_to = transition;

                    // {post} assigned before any calls to 'walker.increment_post_by()'
                    let text = Token::new(LexType::Text, Source::Range(start, post));
                    walker.increment_post_by(skip_amt); // After/before {text} does not matter
                    return Ok(Some(text));
                }

                if let Some((_, _, p)) = walker.advance() {
                    post = p;
                } else {
                    break;
                }
            }
            // Last token till the end of the file is a 'Text' lexeme.
            let text = Token::new(LexType::Text, Source::Range(start, post));
            fsm.mode = CellMode::Transition(None); // Finish 'process()'
            Ok(Some(text))
        }

        // Way to queue a second push
        // A pull reaching the end of block, means the current block is pulled
        // But we also know that the block ender should be pushed as well, e.g.
        //     'hello {|'
        // means that
        //     LexType::Text 'hello '
        //     LexType::HereDocStart '{|'
        // should be pushed. But pull parser means we just pull one token at
        // a time, so we need to queue a second one.
        CellMode::Transition(t) => {
            let token = replace(t, None);
            // Does not matter what we replace it with
            fsm.mode = replace(&mut fsm.transition_to, CellMode::Text);
            Ok(token)
        }

        CellMode::Comment => {
            let start = walker.post; // Just after the "{#"

            //let (mut curr, mut ch) = (start, fsm.comment.1.chars().next().unwrap());
            let mut post = start;
            loop {
                if walker.original[post..].starts_with(cfg.comment.1) {
                    walker.increment_post_by(cfg.comment.1.len());
                    fsm.mode = CellMode::Text;
                    let text = Token::new(LexType::BlockComment, Source::Range(start, post));
                    return Ok(Some(text));
                }
                if let Some((_, _, p)) = walker.advance() {
                    post = p;
                } else {
                    break;
                }

            }

            let source = Source::Range(start, walker.post);
            Err(Token::new("Comment block no ending tag", source))
        }

        CellMode::HereDoc => {
            let (t, is_done) = lex_code_body(
                &mut fsm.code_mode,
                walker,
                cfg.heredoc.1,
                LexType::HereDocClose,
            )?;
            if is_done {
                fsm.mode = CellMode::Text;
            }
            Ok(t)
        }
        CellMode::Inline => {
            let (t, is_done) = lex_code_body(
                &mut fsm.code_mode,
                walker,
                cfg.inline.1,
                LexType::InlineClose,
            )?;
            if is_done {
                fsm.mode = CellMode::Text;
            }
            Ok(t)
        }
    }
}

/******************************************************************************
 * Code-cell-level FSM
 ******************************************************************************/
#[derive(Debug)]
enum CodeMode {
    Regular,
    Quote,
}

// First character must be alphabetic
fn is_invalid_second_ident_char(c: char) -> bool {
    c != '_' && (c.is_ascii_punctuation() || c.is_whitespace())
}

fn lex_code_body(
    mode: &mut CodeMode,
    walker: &mut Walker,
    closer_str: &str,
    closer: LexType,
) -> PullResult<(Option<Lexeme>, bool)> {
    // Eat whitespace
    walker.peek_until(|c, _| !c.is_whitespace());

    let (ch, curr, post) = if let Some(x) = walker.advance() {
        x
    } else {
        let source = Source::Range(walker.post, walker.original.len());
        let token = Token::new("Did not terminate code block", source);
        return Err(token);
    };
    //println!("{:?} {:?}", ch, &walker.original[curr..post+10]);

    // Main FSM branching handling
    let (maybe_token_type, finished) = match (&mode, ch) {
        // Everything else
        (CodeMode::Regular, _) if walker.original[curr..].starts_with(closer_str) => {
            debug_assert!(closer_str.len() > 0, "Should have been caught when setting {{Config}}.");
            walker.increment_post_by(curr + closer_str.len() - post);
            (closer, true)
        }
        (CodeMode::Regular, _) if ch.is_ascii_alphabetic() => {
            // First check in 'if'
            debug_assert!(
                !is_invalid_second_ident_char(ch),
                "First char of idents should also satisfy second+ char requirements"
            );
            let ident_post = walker.original[post..]
                .find(is_invalid_second_ident_char)
                .unwrap_or(0);
            let peek_post = walker.post + ident_post;
            if walker.original[peek_post..].starts_with('(') {
                walker.increment_post_by(ident_post + len_utf8!('(' => 1));
                (LexType::IdentParen, false)
            } else {
                walker.increment_post_by(ident_post);
                (LexType::Ident, false)
            }
        }
        (CodeMode::Regular, '|') => (LexType::Pipe, false),
        (CodeMode::Regular, '(') => (LexType::ParenStart, false),
        (CodeMode::Regular, ')') => (LexType::ParenClose, false),
        (CodeMode::Regular, '.') => (LexType::Stdin, false),
        (CodeMode::Regular, ':') => (LexType::KeyValSeparator, false),
        (CodeMode::Regular, ',') => (LexType::ArgSeparator, false),
        (CodeMode::Regular, ';') => (LexType::StmtSeparator, false),
        (CodeMode::Regular, '=') => (LexType::Assign, false),
        (CodeMode::Regular, '"') => {
            *mode = CodeMode::Quote;
            (LexType::QuoteStart, false)
        }

        // Quotation stuff
        (CodeMode::Quote, '"') => {
            *mode = CodeMode::Regular;
            (LexType::QuoteClose, false)
        }
        (CodeMode::Quote, '\\') => {
            // `skip(1)` because we `advance(AHEAD)`. Effectively, we `skip(2)`
            if let Some((ch, _, _)) = walker.advance() {
                match ch {
                    'n' => (LexType::QuoteLiteral("\n"), false),
                    't' => (LexType::QuoteLiteral("\t"), false),
                    '"' => (LexType::QuoteLiteral("\""), false),
                    ' ' | '\n' => (LexType::QuoteLiteral(""), false),
                    _ => {
                        let source = Source::Range(curr, post);
                        let token = Token::new("Missing closing quotation mark", source);
                        return Err(token);
                    }
                }
            } else { // EOF
                let source = Source::Range(curr, walker.original.len());
                let token = Token::new("Missing closing quotation mark", source);
                return Err(token);
            }
        }
        (CodeMode::Quote, _) => {
            let is_found = walker.peek_until(|c, _| c == '"' || c == '\\');
            if is_found {
                (LexType::Text, false)
            } else {
                let source = Source::Range(curr, post); // The quote mark
                let token = Token::new("Missing closing quotation mark", source);
                return Err(token);
            }
        }

        _ => {
            let source = Source::Range(curr, post);
            eprintln!("CodeMode::{:?} {:?}", mode, ch);
            return Err(Token::new("lexer.rs: Invalid Syntax", source));
        }
    };

    let source = Source::Range(curr, walker.post);
    let token = Token::new(maybe_token_type, source);
    Ok((Some(token), finished))
}

/******************************************************************************
 * Functions for use in testing
 ******************************************************************************/
// Remakes the {original} from {lexemes}
fn reconstruct_string(original: &str, lexemes: &[Lexeme]) -> String {
    // @TODO: Input config as an argument
    let config = Config {
        heredoc: ("{|", "|}"),
        inline: ("{$", "$}"),
        comment: ("{#", "#}"),
        literals: &[],
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

            LexType::KeyValSeparator => push_check!(buffer ':' if text == ":"),
            LexType::ArgSeparator => push_check!(buffer ',' if text == ","),
            LexType::StmtSeparator => push_check!(buffer ';' if text == ";"),
            LexType::Assign => push_check!(buffer '=' if text == "="),


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
