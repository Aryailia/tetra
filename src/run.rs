//run: cargo test -- --nocapture

use std::collections::HashMap;
use std::fmt::Debug;

use crate::parser::sexpr::Arg;
use crate::parser::ast::{Command, Label};
use crate::framework::{Source, Token};

#[derive(Clone, Debug)]
pub enum Value<'source, CustomValue> {
    Str(&'source str),
    Char(char),
    String(String),
    Bool(bool),
    List(Vec<Value<'source, CustomValue>>),
    Custom(CustomValue),
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
    original: &'a str,
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
    ctx.register_stateless_function("cite", &concat);
    println!("{:?}", ctx.definitions.get("pass").unwrap().call(&[]));
    println!("====start====");

    let mut storage: Vec<Value<()>> = Vec::with_capacity(ast.len());
    let mut binded_args = Vec::with_capacity(args.len());
    for cmd in ast.iter() {
        let start = binded_args.len();
        //let func = match &cmd.label {
        //    Label::Assign(s) => }
        //    Label::Ident(s) => {
        //        if let Some(func) = ctx.definitions.get(s.to_str(source)) {
        //
        //        } else {
        //            return Err(Token::new("No function named this.", s.clone()));
        //        }
        //    }
        //    Label::Display => {
        //        storage.push(concat(&binded_args[start..binded_args.len()]));
        //    }
        //}

        for arg in &args[cmd.args.0..cmd.args.1] {
            let binding = match arg.me {
                Arg::Str => Value::Str(arg.to_str(original)),
                Arg::Char(c) => Value::Char(c),
                Arg::Ident => {
                    // Function (with no arguments) or variable retrieval
                    let name = arg.source.to_str(original);
                    // Run a command
                    if let Some(func) = ctx.definitions.get(name) {
                        func.call(&[])
                    } else if let Some(var) = ctx.bindings.get(&Id::Ident(name)) {
                        // @TODO: make this not clone?
                        var.clone()
                    } else if !matches!(&cmd.label, Label::Assign(_)) {
                        return Err(Token::new(
                            "This variable has not be defined yet. Define it with\n    '<var> = <value>;'",
                            arg.source.clone(),
                        ));
                    } else {
                        continue;
                    }
                }
                // sexpr and ast parsing steps should have moved these to only
                // appear as {{Command}.label}
                Arg::IdentFunc => unreachable!(),
                Arg::Assign => unreachable!(),
                Arg::Reference(i) => {
                    if storage.len() == i {
                        println!("dying at {:?}", &cmd.label);
                        println!("{:?}", &args[cmd.args.0..cmd.args.1]);
                        println!("dying at {:?}", Arg::Reference(i));
                    }
                    storage[i].clone()
                }
                Arg::Stdin => unreachable!(),
                Arg::PipedStdin => unreachable!(),
                Arg::Pipe => unreachable!(),
            };
            binded_args.push(binding);
        }
        match &cmd.label {
            Label::Assign(_) => {
                let lvalue = &args[cmd.args.0];
                debug_assert!(matches!(lvalue.me, Arg::Ident), "{:?}", lvalue);
                debug_assert_eq!(binded_args.len(), 2);
                let id = Id::Ident(lvalue.source.to_str(original));
                // @TODO: Should this be cloned?
                ctx.bindings.insert(id, binded_args[0].clone());
                storage.push(Value::Str(""));
            }
            Label::Ident(s) => {
                let name = s.to_str(original);
                let close = binded_args.len();
                if start == close && let Some(var) = ctx.bindings.get(&Id::Ident(name)) {
                    storage.push(var.clone())
                } else if let Some(func) = ctx.definitions.get(name) {
                    storage.push(func.call(&binded_args[start..close]))
                } else {
                    Err(Token::new("No function or variable named this.", s.clone()))?;
                }
            }
            Label::Display => {
                storage.push(concat(&binded_args[start..binded_args.len()]));
            }
        }
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
        _ => unreachable!(),
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
    fn call<'a>(&self, args: &[Value<'a, V>]) -> Value<'a, V>;
}

impl<F, V> OneTimeFunction<V> for F
where
    F: for<'a> Fn(&[Value<'a, V>]) -> Value<'a, V> + Sync + Send,
{
    fn call<'a>(&self, args: &[Value<'a, V>]) -> Value<'a, V> {
        self(args)
    }
}

pub fn pass<'a, V: Debug>(_args: &[Value<'a, V>]) -> Value<'a, V> {
    Value::Str("===Value===")
}

pub fn concat<'a, V: Debug>(args: &[Value<'a, V>]) -> Value<'a, V> {
    let mut buffer = String::with_capacity(recursive_calc_length(args));
    recursive_concat::<V>(args, &mut buffer);
    Value::String(buffer)
}

fn recursive_calc_length<V>(args: &[Value<V>]) -> usize {
    let mut sum = 0;
    for a in args {
        sum += match a {
            Value::Str(s) => s.len(),
            Value::Char(c) => c.len_utf8(),
            Value::String(s) => s.len(),
            Value::Bool(b) => b.then(|| "true").unwrap_or("false").len(),
            Value::List(l) => recursive_calc_length(l),
            Value::Custom(_) => 0,
        };
    }
    sum
}

fn recursive_concat<'a, V: Debug>(args: &[Value<'a, V>], buffer: &mut String) {
    for arg in args {
        match arg {
            Value::Str(s) => buffer.push_str(s),
            Value::Char(c) => buffer.push(*c),
            Value::String(s) => buffer.push_str(s.as_str()),
            Value::Bool(b) => buffer.push_str(b.then(|| "true").unwrap_or("false")),
            Value::List(l) => recursive_concat(l, buffer),
            Value::Custom(_) => todo!(),
        };
    }
}
