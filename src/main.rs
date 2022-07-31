use std::fs;
use std::io;
use std::io::{Read, Write};

use tetra::{run, api::{FileType, Metadata}};
//use xflags;

// https://fuchsia.dev/fuchsia-src/development/api/cli#keyed_options
// NOTE: Fuschia suggests not using - as an alias for STDIN

mod flags {
    #![allow(unused)]

    xflags::xflags! {
        /// Runs the Tetra parser on the file of your choice
        cmd tetra {

            /////
            //optional --dry-run

            /// Prints the help message
            optional -h, --help

            /// Parse tree
            cmd parse
                ///
                required input_file: String

                ///
                optional output_file: String
            {
            }

            /// Runs this on the stdin
            cmd parse-stdin
                /////
                optional output_file: String
            {
            }
        }
    }
}

//run: cargo run -- parse a
fn main() {
    //let bang = if flags.emoji { "❣️" } else { "!" };

    let (contents, target_file) = match flags::Tetra::from_env() {
        Ok(flags) => match flags.subcommand {
            flags::TetraCmd::Parse(p) => {
                match fs::read_to_string(&p.input_file) {
                    Ok(s) => (s, p.output_file),
                    Err(err) => {
                        eprintln!("{}. {:?}", err, p.input_file);
                        std::process::exit(1)
                    }
                }
            }
            flags::TetraCmd::ParseStdin(p) => {
                let mut s = String::new();
                match io::stdin().read_to_string(&mut s) {
                    Ok(_) => (s, p.output_file),
                    Err(err) => {
                        eprintln!("Could not read from STDIN. {}", err);
                        std::process::exit(1)
                    }
                }
            }
        }
        Err(err) => {
            eprintln!("{}\n{}", err, flags::Tetra::HELP);
            std::process::exit(1)
        }
    };

    let ctx = run::markup::default_context();
    if let Some(path) = target_file {

        let output = match ctx.compile(&contents, Metadata::new(FileType::Default, FileType::from(path.as_str()))) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        };
        let mut buffer = fs::File::create(&path).unwrap();
        buffer.write(output.as_bytes()).unwrap();
    } else {
        match ctx.compile(&contents, Metadata::new(FileType::Default, FileType::AsciiDoctor)) {
            Ok(s) => println!("{}", s),
            Err(err) => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        };
    }
}
