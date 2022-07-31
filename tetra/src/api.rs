//run: cargo test -- --nocapture

//use std::collections::HashMap;

// Maybe see OPML spec for design
// {usize} is the level, so we can pack it into an array
#[derive(Debug)]
pub struct OutlineEntry<'source>(usize, &'source str);

#[derive(Clone, Debug)]
pub enum FileType {
    AsciiDoctor,
    Markdown,
    CommonMark,
    RMarkdown,

    LaTeX,
    PDF,
    HTML,

    Default,
    Custom(String),
}

impl From<&str> for FileType {
    fn from(path: &str) -> FileType {
        macro_rules! map {
            ($ext:expr, $( $str:literal => $ret:expr, )* ) => {
                {
                }
                $( if $ext.eq_ignore_ascii_case($str) {
                    $ret
                } else )* {
                    FileType::Default
                }
            }
        }

        if let Some(i) = path.rfind('.').map(|i| i + len_utf8!('.' => 1)) {
            map! { path[i..],
                "adoc" => FileType::AsciiDoctor,
                "asciidoc" => FileType::AsciiDoctor,
                "md" => FileType::CommonMark,
                "rmd" => FileType::RMarkdown,
                "tex" => FileType::LaTeX,
                "pdf" => FileType::PDF,
                "html" => FileType::HTML,
            }
        } else {
            FileType::Default
        }
    }
}


// Metadata
#[derive(Clone, Debug)]
pub struct Metadata {
    pub input_filetype: FileType,
    pub output_filetype: FileType,
    //build_command: String,
}

impl Metadata{
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
    pub meta: Metadata,
    source: &'source str,
    //opts: &HashMap<(usize, K), V>,
    id: usize, // id for ooptions
    pub outline: Vec<OutlineEntry<'source>>,
}

impl<'source> Api<'source> {
    pub fn new(source: &'source str, id: usize, meta: &Metadata) -> Self {
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
