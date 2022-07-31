//run: cargo test -- --nocapture

use std::borrow::Cow;

use super::{Bindings, Error, Value, ValueRepr, Variables, VALUE_AS_STR};
use crate::api::Api;

////////////////////////////////////////////////////////////////////////////////
pub const LIMITED: bool = true;
pub const UNLIMITED: bool = false;
pub type DirtyValue<'a, CustomValue> = (Dirty, Value<'a, CustomValue>);
pub type StatefulResult<'a, V> = Result<DirtyValue<'a, V>, Error>;
pub type PureResult<'a, V> = Result<Value<'a, V>, Error>;

#[derive(Clone, Debug)]
pub enum Dirty {
    Waiting,
    Ready,
}

impl<'a, K, V> Bindings<'a, K, V> {
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
            Func::Pure(
                f,
                ParamDef {
                    parameters: (len, self.parameters.len()),
                    arg_count: if limit_args == LIMITED {
                        (len, len)
                    } else {
                        (0, usize::MAX)
                    },
                },
            ),
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
            Func::Stateful(
                f,
                ParamDef {
                    parameters: (len, self.parameters.len()),
                    arg_count: if limit_args == LIMITED {
                        (len, len)
                    } else {
                        (0, usize::MAX)
                    },
                },
            ),
        );
    }
}

////////////////////////////////////////////////////////////////////////////////
// Custom Functions
// {K} is a custom key enum, {V} is a custom value enum

pub enum Func<'a, K, V> {
    Pure(&'a dyn PureFunction<V>, ParamDef),
    Stateful(&'a dyn StatefulFunction<K, V>, ParamDef),
}

pub struct ParamDef {
    parameters: (usize, usize),
    arg_count: (usize, usize),
}

// @TODO: check is excuted on every iteration, it might be possible to check
//        only once if the arguments have not changed
impl ParamDef {
    pub fn check_args<V>(&self, all_params: &[ValueRepr], args: &[Value<V>]) -> Result<(), Error> {
        let parameters = &all_params[self.parameters.0..self.parameters.1];
        //println!("{:?} {:?} {:?}", all_params, self.arg_count, self.parameters);

        if args.len() < self.arg_count.0 {
            return Err(args
                .len()
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
            return Err(args
                .len()
                .checked_sub(1)
                .map(|i| Error::Arg(i, Cow::Borrowed("Unexpected argument")))
                // Give to label context if no args
                .unwrap_or(Error::Generic(Cow::Borrowed("Unexpected argument"))));
        } else {
            for (i, (a1, a2)) in parameters.iter().zip(args.iter()).enumerate() {
                if *a1 != a2.tag() {
                    return Err(Error::Arg(
                        i,
                        Cow::Owned(format!(
                            "is a value of type {}. Expected a {}",
                            VALUE_AS_STR[*a1 as usize],
                            VALUE_AS_STR[a2.tag() as usize],
                        )),
                    ));
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
    fn call<'a>(&self, args: &[Value<'a, V>], api: Api<'a>) -> PureResult<'a, V>;
}

// Ditto 'PureFunction<V>'
pub trait StatefulFunction<K, V>: Sync + Send {
    fn call<'a>(
        &self,
        args: &[Value<'a, V>],
        api: Api<'a>,
        // This is the value that the previous epoch/iteration call of this
        // function outputted it.
        // On the first iteration, this defaults to Value::Null
        old_output: Value<'a, V>,
        storage: &mut Variables<'a, K, V>,
    ) -> StatefulResult<'a, V>;
}

impl<F, V> PureFunction<V> for F
where
    F: for<'a> Fn(&[Value<'a, V>], Api<'a>) -> PureResult<'a, V> + Sync + Send,
{
    fn call<'a>(&self, args: &[Value<'a, V>], api: Api<'a>) -> PureResult<'a, V> {
        self(args, api)
    }
}

impl<F, K, V> StatefulFunction<K, V> for F
where
    F: for<'a> Fn(
            &[Value<'a, V>],
            Api<'a>,
            Value<'a, V>,
            &mut Variables<'a, K, V>,
        ) -> StatefulResult<'a, V>
        + Sync
        + Send,
{
    fn call<'a>(
        &self,
        args: &[Value<'a, V>],
        api: Api<'a>,
        old_output: Value<'a, V>,
        storage: &mut Variables<'a, K, V>,
    ) -> StatefulResult<'a, V> {
        self(args, api, old_output, storage)
    }
}
