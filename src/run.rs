//run: cargo test -- --nocapture

use std::fmt::Debug;
use std::collections::HashMap;

use crate::framework::{Source, Token};
use crate::sexpr::Arg;
use crate::ast::Command;

#[derive(Clone, Debug)]
pub enum Value<'source, V> {
    Str(&'source str),
    Char(char),
    String(String),
    Bool(bool),
    List(Vec<Value<'source, V>>),
    Custom(V),
}

#[derive(Debug, Eq, Hash, PartialEq)]
enum Id<'source, CustomKey> {
    Ident(&'source str),
    Temp(usize),
    User(CustomKey), // User defined keys (make sure to impl Hash)
}
#[derive(Debug, Eq, Hash, PartialEq)]
enum CustomKey {
    Citation,
}



pub fn run<'a>(
    ast: &[Command],
    args: &[Token<Arg>],
    _function_list: &[&'a str],
    source: &'a str,
) -> Result<(), Token<&'static str>> {
    let mut storage: HashMap<Id<CustomKey>, String> = HashMap::new();
    storage.insert(Id::Ident("hello"), "Hello".to_string());
    println!("{:?}", storage);

    let mut ctx: Context<(), ()> = Context {
        definitions: HashMap::new(),
        bindings: HashMap::new(),
    };
    ctx.register_stateless_function("pass", &pass);
    ctx.register_stateless_function("env", &concat);
    ctx.register_stateless_function("include", &concat);
    ctx.register_stateless_function("run", &pass);
    ctx.register_stateless_function("prettify", &concat);
    ctx.register_stateless_function("end", &pass);
    ctx.register_stateless_function("if", &concat);
    ctx.register_stateless_function("endif", &concat);
    println!("{:?}", ctx.definitions.get("pass").unwrap().call(&mut []));
    println!("====start====");

    let mut storage: Vec<Value<()>> = Vec::with_capacity(ast.len());
    let mut binded_args = Vec::with_capacity(args.len());
    for cmd in ast.iter() {
        let start = binded_args.len();
        for arg in &args[cmd.args.0..cmd.args.1] {
            let binding = match arg.me {
                Arg::Str => Value::Str(arg.to_str(source)),
                Arg::Char(c) => Value::Char(c),
                Arg::Unknown => { // Function (with no arguments) or variable retrieval
                    let name = arg.source.to_str(source);
                    // Run a command
                    if let Some(func) = ctx.definitions.get(name)  {
                        func.call(&[])
                    } else if let Some(_var) = ctx.bindings.get(&Id::Ident(name)) {
                        return Err(Token::new("No support for variables yet.", arg.source.clone()));
                    } else {
                        return Err(Token::new("No support for variables yet.", arg.source.clone()));
                    }
                }
                Arg::Reference(i) => storage[i].clone(),
                Arg::Stdin => unreachable!(),
                Arg::PipedStdin => unreachable!(),
                Arg::Pipe => unreachable!(),
            };
            binded_args.push(binding);
        }
        match &cmd.label {
            Some(s) => {
                if let Some(func) = ctx.definitions.get(s.to_str(source)) {
                    storage.push(func.call(&binded_args[start..binded_args.len()]));
                } else {
                    return Err(Token::new("No function named this.", s.clone()));
                }
            }
            None => {
                storage.push(concat(&binded_args[start..binded_args.len()]));
            }
        }
        //if let Some(label) = cmd.label.as_ref().map(|s| s.to_str(source)) {
        //    //storage.push(Value::Str(label));
        //    i
        //    storage.push(concat(&binded_args[start..binded_args.len()]));
        //} else {
        //    storage.push(concat(&binded_args[start..binded_args.len()]));
        //}
    }

    //let mut storage: Vec<String> = Vec::with_capacity(ast.len());
    //for (i, cmd) in ast.iter().enumerate() {
    //    let label = cmd.label.as_ref().map(|s| s.to_str(source));
    //    let mut output = String::new();
    //    for arg in &args[cmd.args.0..cmd.args.1] {
    //        match arg.me {
    //            Arg::Str => output.push_str(arg.to_str(source)),
    //            Arg::Char(c) => output.push(c),
    //            Arg::Unknown => {}
    //            Arg::Reference(i) => output.push_str(&storage[i]),
    //            Arg::Stdin => unreachable!(),
    //            Arg::PipedStdin => unreachable!(),
    //            Arg::Pipe => unreachable!(),
    //        }
    //    }
    //    storage.push(output);
    //}

    //println!("====");
    //for s in &storage {
    //    println!("{:?}", s);
    //}
    //println!("====");
    match &storage[storage.len() - 1] {
        Value::String(s) => println!("{}", s),
        _ => unreachable!()
    }

    Err(Token::new("Finished parsing", Source::Range(0, 0)))
}

////////////////////////////////////////////////////////////////////////////////
// Custom Functions
// {K} is a custom key enum, {V} is a custom value enum
pub struct Context<'a, 'source, K, V> {
    definitions: HashMap<&'a str, &'a dyn OneTimeFunction<V>>,
    bindings: HashMap<Id<'source, K>, Value<'source, V>>,
}
impl<'a, 'source, K, V> Context<'a, 'source, K, V> {
    pub fn register_stateless_function<F: OneTimeFunction<V> + 'static>(
        &mut self,
        name: &'a str,
        f: &'a F,
    ) {
        self.definitions.insert(name, f);
    }
}

pub trait OneTimeFunction<V>: Sync + Send {
    fn call<'a>(
        &self,
        args: &[Value<'a, V>],
    ) -> Value<'a, V>;
}

impl<F, V> OneTimeFunction<V> for F
where
    F: for<'a> Fn(&[Value<'a, V>]) -> Value<'a, V> + Sync + Send,
{
    fn call<'a>(&self, args: &[Value<'a, V>]) -> Value<'a, V> {
        self(args)
    }
}

pub fn pass<'a, T: Debug>(
    args: &[Value<'a, T>],
) -> Value<'a, T> {
    Value::Str("===Value===")
}


pub fn concat<'a, V: Debug>(
    args: &[Value<'a, V>],
) -> Value<'a, V> {
    let mut buffer = String::with_capacity(recursive_calc_length(args));
    recursive_concat::<V>(args, &mut buffer);
    Value::String(buffer)
}

fn recursive_calc_length<'a, V: Debug>(
    args: &[Value<'a, V>],
) -> usize {
    let mut sum = 0;
    for a in args {
        sum += match a {
            Value::Str(s) => s.len(),
            Value::Char(c) => c.len_utf8(),
            Value::String(s) => s.len(),
            Value::Bool(b) => b.then(|| "true").unwrap_or("false").len(),
            Value::List(l) => recursive_calc_length(&args),
            Value::Custom(_) => 0,
        };
    }
    sum
}

fn recursive_concat<'a, V: Debug>(
    args: &[Value<'a, V>],
    buffer: &mut String,
) {
    for arg in args {
        match arg {
            Value::Str(s) => buffer.push_str(s),
            Value::Char(c) => buffer.push(*c),
            Value::String(s) => buffer.push_str(s.as_str()),
            Value::Bool(b) => buffer.push_str(b.then(|| "true").unwrap_or("false")),
            Value::List(l) => recursive_concat(&l, buffer),
            Value::Custom(_) => todo!(),
        };
    }
}
