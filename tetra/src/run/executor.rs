//run: cargo test -- --nocapture

use std::collections::HashMap;
use std::mem;

use super::{Bindings, Dirty, DirtyValue, Func, MyError, PureResult, Value, Variables};

use crate::framework::Token;
use crate::parser::ast::{Command, Label};
use crate::parser::sexpr::Arg;

//#[derive(Debug, Eq, Hash, PartialEq)]
//enum Id<'source, CustomKey> {
//    Ident(&'source str),
//    Temp(usize),
//    User(CustomKey), // User defined keys (make sure to impl Hash)
//}

const ITERATION_LIMIT: usize = 1000;

pub fn run<'source, K, V: Clone>(
    ctx: &Bindings<K, V>,
    ast: &[Command],
    args: &[Token<Arg>],
    original: &'source str,
) -> Result<String, Token<&'static str>> {
    let mut internal: HashMap<&str, Value<'source, V>> = HashMap::new();
    let mut external = Variables {
        bindings: HashMap::new(),
    };
    let mut outputs: Vec<DirtyValue<'source, V>> = Vec::with_capacity(ast.len());
    let mut binded_args = Vec::with_capacity(args.len());

    debug_assert!(!ast.is_empty());
    for cmd in ast.iter() {
        cmd.init_args(original, args, &mut binded_args);
        //if let Label::Assign(_) = cmd.label {
        //    outputs.push((Dirty::Ready, Value::Null));
        //} else {
        outputs.push((Dirty::Waiting, Value::Null));
        //}
    }
    //println!("{:?}", binded_args);

    let last_index = outputs.len() - 1;
    let mut iter_count = 0;
    while let Dirty::Waiting = outputs[last_index].0 {
        for (i, cmd) in ast.iter().enumerate() {
            if cmd.are_args_ready(args, &outputs) {
                cmd.load_args(ast, args, &mut binded_args, &mut outputs);
            } else {
                //panic!("\n    {}\n", cmd.to_display(args, original));
                continue;
            }

            let bindings = &binded_args[cmd.args.0..cmd.args.1];
            match &cmd.label {
                Label::Assign(_) => {
                    let lvalue = &args[cmd.args.0];
                    let name = lvalue.source.to_str(original);
                    debug_assert!(matches!(lvalue.me, Arg::Ident), "{:?}", lvalue);
                    debug_assert_eq!(2, bindings.len());

                    if ctx.functions.get(name).is_some() {
                        return Err(Token::new("A function with this name already exists. Choose a different name for this variable.", lvalue.source.clone()));
                    }

                    // @TODO: Should this be cloned?
                    internal.insert(name, bindings[1].clone());
                    outputs[i] = (Dirty::Ready, bindings[1].clone());
                }
                Label::Ident(s) => {
                    let name = s.to_str(original);
                    if let Some(var) = internal.get_mut(name) {
                        outputs[i] = if ast[i].reverse_dependant_count() == 0 {
                            (Dirty::Ready, mem::replace(var, Value::Null))
                        } else {
                            (Dirty::Ready, var.clone())
                        };
                    } else if let Some(func) = ctx.functions.get(name) {
                        let output = match func {
                            Func::Pure(f) => (
                                Dirty::Ready,
                                f.call(bindings).map_err(|err| Token::new(err, s.clone()))?,
                            ),
                            Func::Stateful(f) => {
                                let old_output = mem::replace(&mut outputs[i].1, Value::Null);
                                f.call(bindings, old_output, &mut external)
                                    .map_err(|err| Token::new(err, s.clone()))?
                            }
                        };
                        outputs[i] = output;
                        //outputs[i] = (Dirty::Ready, Value::Str("|"));
                    } else {
                        return Err(Token::new("No function or variable named this.", s.clone()));
                    }
                }
                Label::Display => {
                    // @TODO: have errors return which argument is bad
                    let output = concat(&binded_args[cmd.args.0..cmd.args.1])
                        .map_err(|e| Token::new(e, args[cmd.args.0].source.clone()))?;
                    outputs[i] = (Dirty::Ready, output);
                }
            }
        }

        iter_count += 1;
        if iter_count > ITERATION_LIMIT {
            break;
        }
    }
    //println!("====");
    ////binded_args.iter().for_each(|p| println!("{:?}", p));
    //outputs.iter().for_each(|p| println!("{:?}", p));

    //println!("It took {} iteration(s) to parse", iter_count);
    //println!("====start====");
    match outputs.pop() {
        Some((_, Value::String(s))) => Ok(s),
        _ => unreachable!(),
    }
}

////////////////////////////////////////////////////////////////////////////////
// Helper functions

impl Command {
    fn reverse_dependant_count(&self) -> usize {
        self.provides_for.1 - self.provides_for.0
    }

    // The reason we separate out init from load step is because we have to loop
    // several times if there are stateful commands, repeating the load half
    // Also rust is RAII, so
    fn init_args<'a, V>(
        &self,
        original: &'a str,
        args: &[Token<Arg>],
        bindings: &mut Vec<Value<'a, V>>,
    ) {
        for arg in &args[self.args.0..self.args.1] {
            bindings.push(match arg.me {
                Arg::Str => Value::Str(arg.to_str(original)),
                Arg::Char(c) => Value::Char(c),
                Arg::Reference(_) => Value::Null,
                //Arg::Reference
                Arg::Ident => Value::Null, // First arg of assign is the only place
                Arg::IdentFunc | Arg::Assign | Arg::Stdin => unreachable!(),
            });
        }
    }

    fn are_args_ready<V>(&self, args: &[Token<Arg>], outputs: &[DirtyValue<V>]) -> bool {
        let mut is_ready = true;
        for arg in &args[self.args.0..self.args.1] {
            if let Arg::Reference(j) = arg.me {
                is_ready &= matches!(outputs[j].0, Dirty::Ready);
            }
        }
        is_ready
    }

    fn load_args<'a, V: Clone>(
        &self,
        ast: &[Command],
        args: &[Token<Arg>],
        bindings: &mut [Value<'a, V>],
        outputs: &mut [DirtyValue<'a, V>],
    ) {
        let start = self.args.0;
        for (i, arg) in args[start..self.args.1].iter().enumerate() {
            if let Arg::Reference(j) = arg.me {
                // If {outputs[j]} has no dependents, we can just steal it
                bindings[start + i] = if ast[j].reverse_dependant_count() == 0 {
                    mem::replace(&mut outputs[j].1, Value::Null)

                // Otherwise we have to clone
                } else {
                    outputs[j].1.clone()
                }
            }
        }
    }
}

/******************************************************************************
 * In-built Commands
 ******************************************************************************/
// Just joins its arguments into a string
// Also doubles as the default push to the final knit
pub fn concat<'a, V>(args: &[Value<'a, V>]) -> PureResult<'a, V> {
    let mut buffer = String::with_capacity(recursive_calc_length(args)?);
    recursive_concat::<V>(args, &mut buffer);
    Ok(Value::String(buffer))
}

fn recursive_calc_length<V>(args: &[Value<V>]) -> Result<usize, MyError> {
    let mut sum = 0;
    for a in args {
        sum += match a {
            Value::Null => return Err("You left a null unprocessed"),
            Value::Str(s) => s.len(),
            Value::Char(c) => c.len_utf8(),
            Value::Usize(x) => x.to_string().len(),
            Value::String(s) => s.len(),
            Value::Bool(b) => b.then(|| "true").unwrap_or("false").len(),
            Value::List(l) => recursive_calc_length(l)?,
            Value::Custom(_) => todo!(),
        };
    }
    Ok(sum)
}

fn recursive_concat<'a, V>(args: &[Value<'a, V>], buffer: &mut String) {
    for arg in args {
        match arg {
            Value::Null => unreachable!(),
            Value::Str(s) => buffer.push_str(s),
            Value::Char(c) => buffer.push(*c),
            Value::Usize(x) => buffer.push_str(&x.to_string()),
            Value::String(s) => buffer.push_str(s.as_str()),
            Value::Bool(b) => buffer.push_str(b.then(|| "true").unwrap_or("false")),
            Value::List(l) => recursive_concat(l, buffer),
            Value::Custom(_) => todo!(),
        };
    }
}
