//run: <"../../blog/drafts/improving-chinese-japanese-handwriting.adoc" cargo run ../../blog/.cache/langify "improving.adoc"

use std::fs;
use std::path;
use std::io::{self, Read, Write};
use std::collections::HashMap;

use common::{Analyse, FileType, Walker};

mod flags {
    use std::path::PathBuf;

    xflags::xflags! {
        /// Splits STDIN into the files appropriate to the language
        cmd Langify

            /// Output directory
            required output_dir: PathBuf

            /// Output filename. Takes the file extension for comment from here
            required filename: String
        {
            /// If no language (or just 'ALL') is specified for a file, set what
            /// value it should default. If this flag is not specified, it
            /// defaults to 'en' for English.
            optional -d, --default-lang default_lang: String
        }
    }
}

fn main() {
    match flags::Langify::from_env() {
        Ok(mut args) => {
            //println!("{}", args.extension);
            let extension = args.filename
                .rfind(|c| c == '.')
                .map(|i| &args.filename[i + ".".len()..])
                .unwrap_or("");

            let comment_str = FileType::from(extension)
                .unwrap_or(FileType::Default)
                .comment_prefix();
            let default_lang = args.default_lang.as_ref().map(String::as_str).unwrap_or("en");

            let mut input = String::new();
            log(path::Path::new("STDIN"), io::stdin().read_to_string(&mut input));

            for (lang, string) in parse(&input, comment_str, " api_set_lang:") {
                let lang = if lang == "ALL" {
                    default_lang
                } else {
                    lang
                };
                args.output_dir.push(lang);
                log(&args.output_dir, fs::create_dir_all(&args.output_dir));

                args.output_dir.push(&args.filename);
                let mut output = log(&args.output_dir, fs::File::create(&args.output_dir));

                log(&args.output_dir, output.write_all(string.as_bytes()));


                args.output_dir.pop(); // remove {args.filename}
                args.output_dir.pop(); // remove {lang}
                print!("{} ", lang);
            }

        }
        Err(err) => {
            eprintln!("{}\n{}", err, flags::Langify::HELP);
            std::process::exit(1)
        }
    }
}

fn parse<'a>(original: &'a str, comment_str: &str, api_str: &str) -> HashMap<&'a str, String> {
    let mut choices = "ALL";

    let mut lang_split = Vec::new();
    let mut walker = Walker::new('\n', original);
    let (mut prev, (mut ch, mut curr, _)) = ('\n', walker.current());

    let mut cursor = 0;
    let offset = comment_str.len() + api_str.len();

    loop {
        let from_curr = &original[curr..];

        if prev == '\n'
            && from_curr.starts_with(comment_str)
            && from_curr[comment_str.len()..].starts_with(api_str)
        {
            lang_split.push((choices, &original[cursor..curr]));

            walker.peek_until(|c, _| c == '\r' || c == '\n');
            choices = &original[curr + offset..walker.post];
            match walker.peek() {
                Some('\r') => walker.increment_post_by("\r\n".len()),
                Some('\n') => walker.increment_post_by("\n".len()),
                None => {}
                Some(_) => unreachable!(),
            }
            ch = '\n';
            cursor = walker.post;
        }

        if let Some(x) = walker.advance() {
            prev = ch;
            (ch, curr, _) = x;
        } else {
            break;
        }
    }
    lang_split.push((choices, &original[cursor..]));


    // Allocate and count the available languages
    let mut output = HashMap::new();
    let mut max_len = original.len();
    let mut lang_count = 0;
    for (choices, section) in &lang_split {
        for lang in choices.split_ascii_whitespace() {
            if !output.contains_key(lang) {
                output.insert(lang, String::with_capacity(max_len));
                lang_count += 1;
            }
        }
        max_len -= section.len();
    }
    if lang_count > 1 {
        let _all = output.remove("ALL");
        debug_assert!(_all.is_some());
    }


    // Build each language-specific version
    for (choices, section) in &lang_split {
        for lang in choices.split_ascii_whitespace() {
            if lang == "ALL" {
                for (_, buffer) in &mut output {
                    buffer.push_str(section);
                }
            } else {
                output.get_mut(lang).unwrap().push_str(section);
            }
        }
    }

    output
}

fn log<T>(path: &path::Path, result: io::Result<T>) -> T {
    match result {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error with {:?}\n{:?}", path.to_string_lossy(), e);
            std::process::exit(1);
        }
    }
}

