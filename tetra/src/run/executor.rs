//run: cargo test -- --nocapture

use std::borrow::Cow;
use std::collections::HashMap;
use std::mem;

use super::utility::concat;
use super::{Bindings, Dirty, DirtyValue, Func, Value, Variables};

use crate::api::{Api, Config};
use crate::framework::Token;
use crate::parser::{AstOutput, Command, Label, Param};

//#[derive(Debug, Eq, Hash, PartialEq)]
//enum Id<'source, CustomKey> {
//    Ident(&'source str),
//    Temp(usize),
//    User(CustomKey), // User defined keys (make sure to impl Hash)
//}

const ITERATION_LIMIT: usize = 1000;

impl<'a, K, V: Clone> Bindings<'a, K, V> {
    pub fn run(
        &self,
        ast: &AstOutput,
        config: Config,
        original: &str,
    ) -> Result<String, String> {
        run(self, ast, config, original)
    }
}

pub fn run<'a, K, V: Clone>(
    ctx: &Bindings<'a, K, V>,
    AstOutput(ast, args, _): &AstOutput,
    config: Config,
    original: &str,
) -> Result<String, String> {
    let mut internal: HashMap<&str, Value<V>> = HashMap::new();
    let mut external = Variables {
        bindings: HashMap::new(),
    };
    let mut outputs: Vec<DirtyValue<V>> = Vec::with_capacity(ast.len());
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
            match cmd.label.me {
                Label::Assign => {
                    let lvalue = &args[cmd.args.0];
                    let name = lvalue.source.to_str(original);
                    debug_assert!(matches!(lvalue.me, Param::Ident), "{:?}", lvalue);
                    debug_assert_eq!(2, bindings.len());

                    if ctx.functions.get(name).is_some() {
                        return Err(format!("{} {}",
                                lvalue.source.get_context(original),
                                "A function with this name already exists. Choose a different name for this variable."
                                ));
                    }

                    // @TODO: Should this be cloned?
                    internal.insert(name, bindings[1].clone());
                    outputs[i] = (Dirty::Ready, bindings[1].clone());
                }
                Label::Ident | Label::Func => {
                    let name = cmd.label.to_str(original);
                    match (
                        &cmd.label.me,
                        internal.get_mut(name),
                        ctx.functions.get(name),
                    ) {
                        (_, Some(_), Some(_)) => unreachable!(),
                        (Label::Func, Some(_), _) => unreachable!(),

                        (_, None, Some(func)) => {
                            outputs[i] = match func {
                                Func::Pure(f, params) => (
                                    Dirty::Ready,
                                    params
                                        .check_args(&ctx.parameters, bindings)
                                        .and_then(|_| {
                                            f.call(bindings, Api::new(original, i, &config))
                                        })
                                        .map_err(|err| {
                                            err.to_display(
                                                original,
                                                &cmd.label.source,
                                                &args[cmd.args.0..cmd.args.1],
                                            )
                                        })?,
                                ),
                                Func::Stateful(f, params) => {
                                    let old_output = mem::replace(&mut outputs[i].1, Value::Null);
                                    params
                                        .check_args(&ctx.parameters, bindings)
                                        .and_then(|_| {
                                            f.call(
                                                bindings,
                                                Api::new(original, i, &config),
                                                old_output,
                                                &mut external,
                                            )
                                        })
                                        .map_err(|err| {
                                            err.to_display(
                                                original,
                                                &cmd.label.source,
                                                &args[cmd.args.0..cmd.args.1],
                                            )
                                        })?
                                }
                            }
                        }
                        (_, Some(var), None) => {
                            outputs[i] = if ast[i].reverse_dependant_count() == 0 {
                                (Dirty::Ready, mem::replace(var, Value::Null))
                            } else {
                                (Dirty::Ready, var.clone())
                            };
                        }
                        _ => {
                            return Err(format!(
                                "{} {}",
                                cmd.label.get_context(original),
                                "No function or variable named this.",
                            ))
                        }
                    }
                }
                Label::Concat => {
                    // @TODO: have errors return which argument is bad
                    let output =
                        concat(bindings, Api::new(original, i, &config)).map_err(|e| {
                            e.to_display(original, &cmd.label.source, &args[cmd.args.0..cmd.args.1])
                        })?;
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
        Some((_, Value::Text(s))) => Ok(s.to_string()),
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
    pub fn init_args<'a, V>(
        &self,
        original: &'a str,
        args: &[Token<Param>],
        bindings: &mut Vec<Value<'a, V>>,
    ) {
        for arg in &args[self.args.0..self.args.1] {
            bindings.push(match arg.me {
                Param::Str => Value::Text(Cow::Borrowed(arg.to_str(original))),
                Param::Literal(s) => Value::Text(Cow::Borrowed(s)),
                Param::Ident => Value::Null, // First arg of assign is the only place
                Param::Reference(_) => Value::Null,
                Param::Key => todo!("Not sure how to deal with optional arguments yet."),
                //Param::Reference
            });
        }
    }

    fn are_args_ready<V>(&self, args: &[Token<Param>], outputs: &[DirtyValue<V>]) -> bool {
        let mut is_ready = true;
        for arg in &args[self.args.0..self.args.1] {
            if let Param::Reference(j) = arg.me {
                is_ready &= matches!(outputs[j].0, Dirty::Ready);
            }
        }
        is_ready
    }

    fn load_args<'a, V: Clone>(
        &self,
        ast: &[Command],
        args: &[Token<Param>],
        bindings: &mut [Value<'a, V>],
        outputs: &mut [DirtyValue<'a, V>],
    ) {
        let start = self.args.0;
        for (i, arg) in args[start..self.args.1].iter().enumerate() {
            if let Param::Reference(j) = arg.me {
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
