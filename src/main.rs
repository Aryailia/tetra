use std::env;
use std::fs;
use std::io::{self, Read, Write};

use tetra::{
    self as tetralib,
    api::{Analyse, FileType, Config},
};
//use xflags;

// https://fuchsia.dev/fuchsia-src/development/api/cli#keyed_options
// NOTE: Fuschia suggests not using - as an alias for STDIN

mod flags {
    #![allow(unused)]
    xflags::xflags! {
        /// Runs the Tetra parser on the file of your choice
        cmd tetra

        {

            /////
            //optional --dry-run

            /// Prints the help message
            optional -h, --help

            ///// blah
            optional -i, --input-type input_type: String

            ///// Sets the filetype of
            optional -o, --output-type output_type: String

            /// Parse tree
            cmd parse
                ///
                required inp_path: String

                ///
                optional out_path: String
            {
            }

            /// Runs this on the stdin
            cmd parse-stdin
                /////
                optional out_path: String
            {
            }

            /// Same as the 'parse' subcommand, but print out some metadata as JSON
            cmd parse-and-json
                // Cannot make the first argument optional

                ///
                required inp_path: String

                ///
                required out_path: String
            {}
        }
    }
}

//run: cargo run -- parse-and-json ../readme-source.md /dev/null | jq
fn main() {
    // Process global flags first
    let (inp_filetype, out_filetype, subcommands) = match flags::Tetra::from_env() {
        Ok(args) if args.help => {
            eprintln!("{}", flags::Tetra::HELP);
            std::process::exit(1)
        }
        Ok(args) => {
            // If we explicitly set an invalid filetype, error (with list of valid ones)
            let inp = args.input_type.as_ref().map(|ext| {
                FileType::from(ext).unwrap_or_else(|| {
                    eprintln!("{} is an unsupported file type", ext);
                    std::process::exit(1);
                })
            });

            let out = args.output_type.as_ref().map(|ext| {
                FileType::from(ext).unwrap_or_else(|| {
                    eprintln!("{} is an unsupported file type", ext);
                    std::process::exit(1);
                })
            });
            (inp, out, args.subcommand)
        }
        Err(err) => {
            eprintln!("{}\n{}", err, flags::Tetra::HELP);
            std::process::exit(1)
        }
    };

    // Intepret the subcommands
    let (inp_path, out_path, is_print_json) = match subcommands {
        flags::TetraCmd::Parse(p) => (Some(p.inp_path), p.out_path, false),
        flags::TetraCmd::ParseStdin(p) => (None, p.out_path, false),
        flags::TetraCmd::ParseAndJson(p) => (Some(p.inp_path), Some(p.out_path), true),
    };


    // Read the file from STDIN or {inp_path}, setting {inp_filetype} if appropriate
    let (inp_content, inp_filetype) = if let Some(path) = inp_path {
        // Prefer the '--input-type' switch override. Else find it from {path}
        let ft = inp_filetype.unwrap_or_else(|| path
            .rfind(|c| c == '.')
            .and_then(|i| FileType::from(&path[i + 1..]))
            // No extension or extension not supported, just use 'FileType::Default'
            .unwrap_or(FileType::Default)
        );
        (log(&path, fs::read_to_string(&path)), ft)
    } else {
        let mut stdin = String::new();
        log("STDIN", io::stdin().read_to_string(&mut stdin));
        (stdin, FileType::Default)
    };

    // Set the {out_filetype} if not overridden by the '--output-type' switch
    let out_filetype = out_filetype.unwrap_or_else(|| {
        out_path
            .as_ref()
            .and_then(|path| path
                .rfind(|c| c == '.')
                .and_then(|i| FileType::from(&path[i + 1..]))
            )
            // No extension or extension not supported, just use 'FileType::Default'
            .unwrap_or(FileType::Default)
    });


    // Compile
    let ctx = tetralib::default_context();
    let config = Config::new(inp_filetype, out_filetype);
    let out_content = log("compiling", ctx.compile(&inp_content, config));

    // Write to output
    if let Some(path) = out_path {
        let mut buffer = log(&path, fs::File::create(&path));
        log(&path, buffer.write_all(out_content.as_bytes()));

        if is_print_json {
            println!("{}", inp_filetype.metadata(&out_content).to_json());
        }
    } else {
        println!("{}", out_content);
        debug_assert!(!is_print_json);
    }

}

fn log<T, E: std::fmt::Debug>(path: &str, result: Result<T, E>) -> T {
    match result {
        Ok(s) => s,
        Err(e) => {
            eprintln!("tetra-cli {:?}", env::args());
            eprintln!("Error with {:?}\n{:?}", path, e);
            std::process::exit(1);
            //panic!("\n{:?}\n{}", e, e.get_context(original));
        }
        //Err(e) => match e {
        //    CustomErr::Parse(err) => panic!("\nERROR: {:?}\n", err.msg()),
        //    err => panic!("{:?}", err),
        //},
    }
}

