//run: cargo test -- --nocapture
macro_rules! unwrap {
    (or_invalid $value:expr => $variant:pat => $output:expr) => {
        match $value {
            $variant => Ok($output),
            _ => Err(Error::Generic("Invalid type".into())),
        }
    };
    (unreachable $value:expr => $variant:pat => $output:expr) => {
        match $value {
            $variant => $output,
            _ => unreachable!(),
        }
    };
}

mod executor;
//pub mod exec_async;
mod function;
pub mod utility;

use function::{Func};
pub use function::{PureFunction, PureResult, StatefulFunction, StatefulResult};
pub use function::{Dirty, DirtyValue, LIMITED, UNLIMITED};

////////////////////////////////////////////////////////////////////////////////

use crate::api::Metadata;
use crate::framework::Source;
use crate::parser::{self, AstOutput, Param};
use crate::Token;

use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::Hash;

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    Arg(usize, Cow<'static, str>), // Context will be cenetered on the arg indexed at `usize`
    Generic(Cow<'static, str>),    // Context will be the function
    Contextless(Cow<'static, str>), // Do not print the context
}

impl Error {
    // @TODO: consider whether to output Cow<str> or not
    fn to_display(&self, original: &str, label: &Source, args: &[Token<Param>]) -> String {
        println!("{:?} {:?}", args, self);
        match self {
            Error::Arg(i, s) => format!("{} {}", args[*i].source.get_context(original), s),
            Error::Generic(s) => format!("{} {}", label.get_context(original), s),
            Error::Contextless(s) => s.to_string(),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

// This mirrors the behaviour of `std::mem::discriminant()`
// But this automates mapping discriminants back into `str`
macro_rules! define_value {
    (const $const:ident = enum $enum:ident: $repr:ty {
        $($variant_id:ident = $variant:ident $(($type:ty))? , )*
    }) => {
        type ValueRepr = $repr;

        #[derive(Clone, Debug)]
        #[repr($repr)]
        pub enum $enum<'source, CustomValue> {
            $($variant $(($type))*, )*
        }

        pub mod value {
            define_value!{ @consts $repr {0} $($variant_id)* }
        }
        // Just defines a bunch of
        // `const NULL = 0`
        // `const STR = 1` etc.
        // for each {$variant_id} but automatically increments the r-value

        impl<'source, CustomValue> $enum<'source, CustomValue> {
            pub fn tag(&self) -> $repr {
                unsafe { *(self as *const Self as *const $repr) }
            }
        }

        const VALUE_VARIANT_COUNT: usize = 0 $( + define_value!(@count $variant) )*;

        const $const: [&'static str; VALUE_VARIANT_COUNT] = [
            $( stringify!($variant), )*
        ];
        //#[test]
        //fn check_variant_matches_debug_str
    };

    // tt-muncher pattern to count the {$name}
    (@consts $repr:ty { $index:expr } $name:ident $($($tt:tt)+)?) => {
        pub const $name: $repr = $index;
        $( define_value!{ @consts $repr { $index + 1 } $($tt)* })*
    };
    (@count $_:tt) => { 1 };
}

define_value! { const VALUE_AS_STR = enum Value: u8 {
    NULL   = Null,
    TEXT   = Text(Cow<'source, str>),
    USIZE  = Usize(usize),
    CHAR   = Char(char),
    BOOL   = Bool(bool),
    LIST   = List(Vec<Value<'source, CustomValue>>),
    CUSTOM = Custom(CustomValue),
}}

////////////////////////////////////////////////////////////////////////////////

// {Variables} is used by user-defined functions and by the executor internally
//
// We are wrapping the HashMap because we plan on using an arena allocator
// later on and hiding that implementation detail from user-defined functions
pub struct Variables<'source, K, V> {
    bindings: HashMap<K, Value<'source, V>>,
}
impl<'a, K: Eq + Hash, V> Variables<'a, K, V> {
    pub fn get(&self, key: &K) -> Option<&Value<'a, V>> {
        self.bindings.get(key)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut Value<'a, V>> {
        self.bindings.get_mut(key)
    }
    pub fn insert(&mut self, key: K, value: Value<'a, V>) -> Option<Value<'a, V>> {
        self.bindings.insert(key, value)
    }
}

////////////////////////////////////////////////////////////////////////////////
// Main context
pub struct Bindings<'a, K, V> {
    functions: HashMap<&'a str, Func<'a, K, V>>,
    parameters: Vec<ValueRepr>,
}

#[cfg_attr(feature = "cargo-clippy", allow(clippy::new_without_default))]
impl<'a, K, V> Bindings<'a, K, V> {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            parameters: Vec::new(),
        }
    }

    pub fn build(original: &str) -> Result<AstOutput, String> {
        parser::step1_lex(original, true)
            .and_then(|lexemes| parser::step2_to_sexpr(&lexemes, original))
            .and_then(|sexprs| parser::step3_to_ast(&sexprs))
            .map_err(|token| format!("{} {}", token.get_context(original), token.me))
    }

    // in "run/function.rs"
    //pub fn register_pure_function;
    //pub fn register_stateful_function
}

impl<'a, K, V: Clone> Bindings<'a, K, V> {
    // Defined in the "run/executor.rs"
    //pub fn run();

    pub fn compile(&self, original: &str, metadata: Metadata) -> Result<String, String> {
        self.run(&Self::build(original)?, metadata, original)
    }

}
