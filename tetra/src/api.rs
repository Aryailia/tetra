//run: cargo test -- --nocapture

//use std::collections::HashMap;

pub use common::FileType;

// Maybe see OPML spec for design
// {usize} is the level, so we can pack it into an array
#[derive(Debug)]
pub struct OutlineEntry<'source>(usize, &'source str);

// Metadata
#[derive(Clone, Debug)]
pub struct Config {
    pub input_filetype: FileType,
    pub output_filetype: FileType,
    //build_command: String,
}

impl Config {
    pub fn new(input_filetype: FileType, output_filetype: FileType) -> Self {
        Self {
            input_filetype,
            output_filetype,
            //build_command: String::new(),
        }
    }
}

#[derive(Debug)]
pub struct Api<'source> {
    pub meta: Config,
    source: &'source str,
    //opts: &HashMap<(usize, K), V>,
    id: usize, // id for ooptions
    pub outline: Vec<OutlineEntry<'source>>,
}

impl<'source> Api<'source> {
    pub fn new(source: &'source str, id: usize, meta: &Config) -> Self {
        Api {
            meta: meta.clone(),
            source,
            id,
            outline: Vec::new(),
        }
    }
}

//pub trait Api {
//    fn get_metadata(&self);
//    fn optional_parameters(&self) -> ;
//}
