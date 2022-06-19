//run: cargo test -- --nocapture
macro_rules! unwrap {
    (or_invalid $value:expr => $type:ident($x:ident) => $output:expr) => {
        match $value {
            Value::$type($x) => Ok($output),
            _ => Err(Error::Generic("Invalid type".into())),
        }
    };
    (unreachable $value:expr => $type:ident($x:ident) => $output:expr) => {
        match $value {
            Value::$type($x) => $output,
            _ => unreachable!(),
        }
    };
}

mod executor;
pub mod markup;
pub mod utility;

use crate::framework::Source;
use crate::parser::{self, Arg, Command};
use crate::Token;

use std::collections::HashMap;
use std::hash::Hash;
use std::borrow::Cow;

////////////////////////////////////////////////////////////////////////////////

pub enum Error {
    Arg(usize, Cow<'static, str>), // Context will be cenetered on the arg indexed at `usize`
    Generic(Cow<'static, str>), // Context will be the function
    Contextless(Cow<'static, str>), // Do not print the context
}


impl Error {
    // @TODO: consider whether to output Cow<str> or not
    fn to_display(&self, original: &str, label: &Source, args: &[Token<Arg>]) -> String {
        match self {
            Error::Arg(i, s) => format!("{} {}", args[*i].source.get_context(original), s),
            Error::Generic(s) => format!("{} {}", label.get_context(original), s),
            Error::Contextless(s) => s.to_string(),
        }
    }
}



////////////////////////////////////////////////////////////////////////////////

type DirtyValue<'a, CustomValue> = (Dirty, Value<'a, CustomValue>);
pub type StatefulResult<'a, V> = Result<DirtyValue<'a, V>, Error>;
pub type PureResult<'a, V> = Result<Value<'a, V>, Error>;

#[derive(Clone, Debug)]
pub enum Dirty {
    Waiting,
    Ready,
}

#[derive(Clone, Debug)]
pub enum Value<'source, CustomValue> {
    Null,
    Str(&'source str),
    Usize(usize),
    Char(char),
    String(String),
    Bool(bool),
    List(Vec<Value<'source, CustomValue>>),
    Custom(CustomValue),
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

pub struct Bindings<'a, K, V> {
    functions: HashMap<&'a str, Func<'a, K, V>>,
}

impl<'a, K, V: Clone> Bindings<'a, K, V> {
    pub fn run<'source>(
        &self,
        ast: &[Command],
        args: &[Token<Arg>],
        original: &'source str,
    ) -> Result<String, String> {
        executor::run(self, ast, args, original)
    }

    pub fn compile(&self, original: &str) -> Result<String, String> {
        let (ast, args2, _provides_for) = parser::step1_lex(original, true)
            .and_then(|lexemes| parser::step2_to_sexpr(&lexemes, original))
            .and_then(|(sexprs, args1)| parser::step3_to_ast(&sexprs, &args1))
            .map_err(|token| format!("{} {}", token.get_context(original), token.me))?;
        //let lexemes = parser::step1_lex(original, true)?;
        //let (sexprs, args1) = parser::step2_to_sexpr(&lexemes, original)?;
        //let (ast, args2, _provides_for) = parser::step3_to_ast(&sexprs, &args1)?;
        self.run(&ast, &args2, original)
    }
}

////////////////////////////////////////////////////////////////////////////////
// Custom Functions
// {K} is a custom key enum, {V} is a custom value enum

enum Func<'a, K, V> {
    Pure(&'a dyn PureFunction<V>),
    Stateful(&'a dyn StatefulFunction<K, V>),
}

pub trait PureFunction<V>: Sync + Send {
    fn call<'a>(&self, args: &[Value<'a, V>]) -> PureResult<'a, V>;
}

pub trait StatefulFunction<K, V>: Sync + Send {
    fn call<'a>(
        &self,
        args: &[Value<'a, V>],
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

#[cfg_attr(feature = "cargo-clippy", allow(clippy::new_without_default))]
impl<'a, K, V: Clone> Bindings<'a, K, V> {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }
    pub fn register_pure_function<F: PureFunction<V> + 'static>(
        &mut self,
        name: &'a str,
        f: &'a F,
    ) {
        self.functions.insert(name, Func::Pure(f));
    }

    pub fn register_stateful_function<F: StatefulFunction<K, V> + 'static>(
        &mut self,
        name: &'a str,
        f: &'a F,
    ) {
        self.functions.insert(name, Func::Stateful(f));
    }
}
