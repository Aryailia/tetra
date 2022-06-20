// This the default flavour of this templating markup language. If you want
// to implement your own flavour (i.e. with your own functions), you should
// be able to copy this file directly.

//run: cargo test -- --nocapture

use std::borrow::Cow;

// Do not use super so that if others want to make their own flavour, they
// can copy this file without issue
use crate::run::{Bindings, Dirty, Error, PureResult, StatefulResult};
use crate::run::{Value, Variables};

use crate::run::utility::{code, concat, env};
use crate::run::utility::{fetch_env_var, run_command};
use crate::run::{LIMITED, UNLIMITED}; // these are just bools
use crate::run::value as v;


pub fn default_context<'a>() -> Bindings<'a, CustomKey, CustomValue> {
    let mut ctx = Bindings::new();
    ctx.register_pure_function("env", &env, LIMITED, &[v::TEXT]);
    ctx.register_pure_function("include", &concat, UNLIMITED, &[]);

    // "r/run <lang> <code-body>"
    ctx.register_pure_function("run", &code, LIMITED, &[v::TEXT, v::TEXT]);
    ctx.register_pure_function("r", &code, LIMITED, &[v::TEXT, v::TEXT]);

    ctx.register_pure_function("prettify", &concat, LIMITED, &[]);
    ctx.register_pure_function("end", &concat, LIMITED, &[v::TEXT]);
    ctx.register_pure_function("if", &concat, LIMITED, &[]);
    ctx.register_pure_function("endif", &concat, LIMITED, &[v::TEXT]);
    ctx.register_stateful_function("cite", &cite, LIMITED, &[v::TEXT]);
    ctx.register_stateful_function("references", &references, LIMITED, &[]);
    ctx
}

////////////////////////////////////////////////////////////////////////////////
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CustomKey {
    Citations,
    CiteCount,
    CiteState,
}

#[derive(Clone, Debug)]
pub enum CustomValue {
    CiteList(Vec<String>),
    Citation(usize),
}

////////////////////////////////////////////////////////////////////////////////
fn cite<'a>(
    args: &[Value<'a, CustomValue>],
    old_output: Value<'a, CustomValue>,
    storage: &mut Variables<'a, CustomKey, CustomValue>,
) -> StatefulResult<'a, CustomValue> {
    if args.len() != 1 {
        panic!();
    }
    //println!("* {:?} {:?}", &old_output, storage.get(&CustomKey::Citations));
    let old_state = storage
        .get(&CustomKey::CiteState)
        .map(|v| unwrap!(unreachable v => Value::Usize(x) => *x))
        .unwrap_or(0);

    // Determine what the state is for our FSM
    // {old_output} determines what pass (epoch/iteration) we are on
    let (state, id) = match (old_state, &old_output) {
        // All 'cite' calls first pass => count citations
        (0, Value::Null) => (0, 0),

        // First cite call, second pass => create citekey list
        (0, Value::Usize(i)) => (1, *i),

        // Other cite calls, second pass => just append to citekey list
        (1 | 2, Value::Usize(i)) => (2, *i),

        // First cite call, third pass => run pandoc
        (1 | 2, Value::Custom(CustomValue::Citation(i))) => (3, *i),

        // Other cite calls, third pass => just output the citation
        (3 | 4, Value::Custom(CustomValue::Citation(i))) => (4, *i),

        _ => unreachable!("{} {:?}", old_state, &old_output),
    };
    storage.insert(CustomKey::CiteState, Value::Usize(state));

    // Init data structures
    if state == 1 {
        //let cite_count = storage.get(&CustomKey::CiteCount)
        //    .map(|v| unwrap!(or_invalid v => Usize(x) => *x))
        //    .unwrap_or(Ok(0))?;

        // @TODO: String::with_capacity
        storage.insert(CustomKey::Citations, Value::Text(Cow::Owned(String::new())));
    } else if state == 3 {
        let citekeys_value = storage.get(&CustomKey::Citations).unwrap();
        let citekeys = unwrap!(unreachable citekeys_value => Value::Text(s) => s);
        let citerefs = pandoc_cite(citekeys)?;
        storage.insert(CustomKey::Citations, Value::Text(Cow::Owned(citerefs)));
    }

    //println!("| cite step {:?}", step);
    match state {
        0 => {
            let cite_count = storage
                .get(&CustomKey::CiteCount)
                .map(|v| unwrap!(or_invalid v => Value::Usize(x) => *x))
                .unwrap_or(Ok(0))?;
            storage.insert(CustomKey::CiteCount, Value::Usize(cite_count + 1));
            Ok((Dirty::Waiting, Value::Usize(cite_count)))
        }
        1 | 2 => {
            let list_value = storage.get_mut(&CustomKey::Citations).unwrap();
            let list: &mut String = unwrap!(unreachable list_value => Value::Text(Cow::Owned(s)) => s);
            list.push_str(unwrap!(unreachable &args[0] => Value::Text(s) => s));
            list.push('\n');
            list.push('\n');
            Ok((Dirty::Waiting, Value::Custom(CustomValue::Citation(id))))
        }
        3 | 4 => {
            let citerefs = storage.get_mut(&CustomKey::Citations).unwrap();
            let citerefs = unwrap!(unreachable citerefs => Value::Text(s) => s);
            let citation = citerefs.split("\n\n").nth(id).unwrap().to_string();
            Ok((Dirty::Ready, Value::Text(Cow::Owned(citation))))
        }

        _ => unreachable!(),
    }
}

fn references<'a>(
    args: &[Value<'a, CustomValue>],
    _: Value<'a, CustomValue>,
    storage: &mut Variables<'a, CustomKey, CustomValue>,
) -> StatefulResult<'a, CustomValue> {
    assert_eq!(0, args.len());

    let state = storage
        .get(&CustomKey::CiteState)
        .map(|v| unwrap!(unreachable v => Value::Usize(x) => *x))
        .unwrap_or(0);
    match state {
        0 => Ok((Dirty::Waiting, Value::Text(Cow::Borrowed("")))),
        1 | 2 | 3 | 4 => {
            let cite_count = storage
                .get(&CustomKey::CiteCount)
                .map(|v| unwrap!(unreachable v => Value::Usize(x) => *x))
                .unwrap_or(0);

            let citerefs = storage.get_mut(&CustomKey::Citations).unwrap();
            let citerefs = unwrap!(unreachable citerefs => Value::Text(s) => s);
            let ref_start = citerefs.split("\n\n").nth(cite_count).unwrap().as_ptr();
            let references = &citerefs[ref_start as usize - citerefs.as_ptr() as usize..];

            Ok((Dirty::Ready, Value::Text(Cow::Owned(references.to_string()))))
        }
        _ => unreachable!(),
    }
}


pub fn pandoc_cite(citekey: &str) -> Result<String, Error> {
    let bibliography = fetch_env_var("BIBLIOGRAPHY")?;
    let citation = run_command(
        "pandoc",
        Some(citekey),
        //&["--citeproc", "-M", "suppress-bibliography=true", "-t", "plain",
        &["--citeproc", "-t", "plain", "--bibliography", &bibliography],
    )?;

    Ok(citation)
}

