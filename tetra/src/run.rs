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
pub mod markup;
pub mod utility;

use crate::framework::Source;
use crate::parser::{self, Item, Command};
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
    fn to_display(&self, original: &str, label: &Source, args: &[Token<Item>]) -> String {
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

define_value!{ const VALUE_AS_STR = enum Value: u8 {
    NULL   = Null,
    TEXT   = Text(Cow<'source, str>),
    USIZE  = Usize(usize),
    CHAR   = Char(char),
    BOOL   = Bool(bool),
    LIST   = List(Vec<Value<'source, CustomValue>>),
    CUSTOM = Custom(CustomValue),
}}

////////////////////////////////////////////////////////////////////////////////

type DirtyValue<'a, CustomValue> = (Dirty, Value<'a, CustomValue>);
pub type StatefulResult<'a, V> = Result<DirtyValue<'a, V>, Error>;
pub type PureResult<'a, V> = Result<Value<'a, V>, Error>;

#[derive(Clone, Debug)]
pub enum Dirty {
    Waiting,
    Ready,
}

// {Variables} is used by user-defined functions and by the executor internally
//
// We are wrapping the HashMap because we plan on using an arena allocator
// later on and hiding that implementation detail from user-defined functions
pub struct Variables<'source, CustomKey, CustomValue> {
    bindings: HashMap<CustomKey, Value<'source, CustomValue>>,
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

pub const LIMITED: bool = true;
pub const UNLIMITED: bool = false;

#[cfg_attr(feature = "cargo-clippy", allow(clippy::new_without_default))]
impl<'a, K, V: Clone> Bindings<'a, K, V> {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            parameters: Vec::new(),
        }
    }

    pub fn run(
        &self,
        ast: &[Command],
        args: &[Token<Item>],
        original: &str,
    ) -> Result<String, String> {
        executor::run(self, ast, args, original)
    }

    pub fn compile(&self, original: &str) -> Result<String, String> {
        let (ast, args2, _provides_for) = parser::step1_lex(original, true)
            .and_then(|lexemes| parser::step2_to_sexpr(&lexemes, original))
            .and_then(|sexprs| parser::step3_to_ast(&sexprs))
            .map_err(|token| format!("{} {}", token.get_context(original), token.me))?;
        //let lexemes = parser::step1_lex(original, true)?;
        //let (sexprs, args1) = parser::step2_to_sexpr(&lexemes, original)?;
        //let (ast, args2, _provides_for) = parser::step3_to_ast(&sexprs, &args1)?;
        self.run(&ast, &args2, original)
    }

    pub fn register_pure_function<F: PureFunction<V> + 'static>(
        &mut self,
        name: &'a str,
        f: &'a F,
        limit_args: bool,
        parameters: &[ValueRepr],
    ) {
        let len = parameters.len();
        self.parameters.extend(parameters);

        self.functions.insert(
            name,
            Func::Pure(f, ParamDef {
                parameters: (len, self.parameters.len()),
                arg_count: if limit_args == LIMITED {
                    (len, len)
                } else {
                    (0, usize::MAX)
                },
            }),
        );
    }

    pub fn register_stateful_function<F: StatefulFunction<K, V> + 'static>(
        &mut self,
        name: &'a str,
        f: &'a F,
        limit_args: bool,
        parameters: &[ValueRepr],
    ) {
        let len = parameters.len();
        self.parameters.extend(parameters);

        self.functions.insert(
            name,
            Func::Stateful(f, ParamDef {
                parameters: (len, self.parameters.len()),
                arg_count: if limit_args == LIMITED {
                    (len, len)
                } else {
                    (0, usize::MAX)
                },
            }),
        );
    }
}

////////////////////////////////////////////////////////////////////////////////
// Custom Functions
// {K} is a custom key enum, {V} is a custom value enum

enum Func<'a, K, V> {
    Pure(&'a dyn PureFunction<V>, ParamDef),
    Stateful(&'a dyn StatefulFunction<K, V>, ParamDef),
}

struct ParamDef {
    parameters: (usize, usize),
    arg_count: (usize, usize),
}

// @TODO: check is excuted on every iteration, it might be possible to check
//        only once if the arguments have not changed
impl ParamDef {
    fn check_args<V>(
        &self,
        all_params: &[ValueRepr],
        args: &[Value<V>],
    ) -> Result<(), Error> {
        let parameters = &all_params[self.parameters.0..self.parameters.1];
        //println!("{:?} {:?} {:?}", all_params, self.arg_count, self.parameters);

        if args.len() < self.arg_count.0 {
            return Err(args.len()
                .checked_sub(1)
                .map(|i| Error::Arg(i, Cow::Borrowed("Need an argument after this")))

                // Give label context if no args
                .unwrap_or(Error::Generic(Cow::Borrowed("Missing an argument"))));
        } else if args.len() > self.arg_count.1 {
            //match &args[1] {
            //    Value::Null => println!("Missing: Null"),
            //    Value::Str(s) => println!("Missing: {:?}", s),
            //    Value::String(s) => println!("Missing: {:?}", s),
            //    Value::String(s) => println!("Missing: {:?}", s),
            //    Value::Usize(s) => println!("Missing: {:?}", s),
            //    Value::Char(s) => println!("Missing: {:?}", s),
            //    Value::String(s) => println!("Missing: {:?}", s),
            //    Value::Bool(s) => println!("Missing: {:?}", s),
            //    Value::List(_) => todo!(),
            //    Value::Custom(_) => todo!(),
            //}
            return Err(args.len()
                .checked_sub(1)
                .map(|i| Error::Arg(i, Cow::Borrowed("Unexpected argument")))
                // Give to label context if no args
                .unwrap_or(Error::Generic(Cow::Borrowed("Unexpected argument"))));
        } else {
            for (i, (a1, a2)) in parameters.iter().zip(args.iter()).enumerate() {
                if *a1 != a2.tag() {
                    return Err(Error::Arg(i, Cow::Owned(format!(
                        "is a value of type {}. Expected a {}",
                        VALUE_AS_STR[*a1 as usize],
                        VALUE_AS_STR[a2.tag() as usize],
                    ))));
                }
            }
        }
        Ok(())
    }
}

// This mirrors how the the definitions of the user-defined functions should
// look like as well, (i.e. this are the parameters they should have).
// See 'markup.rs' for more explicit example
pub trait PureFunction<V>: Sync + Send {
    fn call<'a>(&self, args: &[Value<'a, V>]) -> PureResult<'a, V>;
}

// Ditto 'PureFunction<V>'
pub trait StatefulFunction<K, V>: Sync + Send {
    fn call<'a>(
        &self,
        args: &[Value<'a, V>],
        // This is the value that the previous epoch/iteration call of this
        // function outputted it.
        // On the first iteration, this defaults to Value::Null
        old_output: Value<'a, V>,
        storage: &mut Variables<'a, K, V>,
    ) -> StatefulResult<'a, V>;
}

impl<F, V> PureFunction<V> for F
where
    F: for<'a> Fn(&[Value<'a, V>]) -> PureResult<'a, V> + Sync + Send,
{
    fn call<'a>(&self, args: &[Value<'a, V>]) -> PureResult<'a, V> {
        self(args)
    }
}

impl<F, K, V> StatefulFunction<K, V> for F
where
    F: for<'a> Fn(&[Value<'a, V>], Value<'a, V>, &mut Variables<'a, K, V>) -> StatefulResult<'a, V>
        + Sync
        + Send,
{
    fn call<'a>(
        &self,
        args: &[Value<'a, V>],
        old_output: Value<'a, V>,
        storage: &mut Variables<'a, K, V>,
    ) -> StatefulResult<'a, V> {
        self(args, old_output, storage)
    }
}
