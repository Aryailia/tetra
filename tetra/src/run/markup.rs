//run: cargo test -- --nocapture

use std::io::Write;
//use std::default::Default;
use std::process;
use std::process::Stdio;

use super::executor::concat;
use super::executor::{Bindings, Dirty, MyError, PureResult, StatefulResult, Value, Variables};

//impl Default for Bindings<'_, CustomKey, CustomValue>  {
//    fn default() -> Self {
//        let mut ctx = Bindings::new();
//        ctx.register_pure_function("env", &concat);
//        ctx.register_pure_function("include", &concat);
//        ctx.register_pure_function("run", &concat);
//        ctx.register_pure_function("prettify", &concat);
//        ctx.register_pure_function("end", &concat);
//        ctx.register_pure_function("if", &concat);
//        ctx.register_pure_function("endif", &concat);
//        //ctx.register_pure_function("cite", &concat);
//        ctx.register_stateful_function("cite", &cite);
//        ctx.register_stateful_function("references", &references);
//        ctx
//    }
//
//}
pub fn default_context<'a>() -> Bindings<'a, CustomKey, CustomValue> {
    let mut ctx = Bindings::new();
    ctx.register_pure_function("env", &env);
    ctx.register_pure_function("include", &concat);
    ctx.register_pure_function("run", &code);
    ctx.register_pure_function("r", &code);
    ctx.register_pure_function("prettify", &concat);
    ctx.register_pure_function("end", &concat);
    ctx.register_pure_function("if", &concat);
    ctx.register_pure_function("endif", &concat);
    //ctx.register_pure_function("cite", &concat);
    ctx.register_stateful_function("cite", &cite);
    ctx.register_stateful_function("references", &references);
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


pub fn code<'a, V>(args: &[Value<'a, V>]) -> PureResult<'a, V> {
    if args.len() > 2 {
        //println!("len {}", args.len());
        todo!("temp panic for when we put actual error handling");
    }

    //let lang = unwrap!(or_invalid args[0] => String(x) | Str(x) => x);
    let lang: &str = match &args[0] {
        Value::String(x) => x,
        Value::Str(x) => x,
        _ => return Err("Invalid type"),
    };
    let cell_body: &str = match &args[1] {
        Value::String(x) => x,
        Value::Str(x) => x,
        _ => return Err("Invalid type"),
    };

    match lang {
        "r" => {
            println!("markup.rs: Running r");
        }
        "graphviz" | "dot" => {
            return run_command(
                "dot",
                Some(cell_body),
                &["-Tsvg"],
            ).map(Value::String)
        }
        "sh" => println!("markup.rs: Running shell"),
        s => todo!("markup.rs: {}", s),
    }

    Ok(Value::String("".to_string()))
}


//fn code<'a>(
//    args: &[Value<'a, CustomValue>],
//    old_output: Value<'a, CustomValue>,
//    storage: &mut Variables<'a, CustomKey, CustomValue>,
//) -> StatefulResult<'a, CustomValue> {
//    Ok((Dirty::Waiting, Value::String("".to_string())))
//}


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
        .map(|v| unwrap!(unreachable v => Usize(x) => *x))
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
        storage.insert(CustomKey::Citations, Value::String(String::new()));
    } else if state == 3 {
        let citekeys_value = storage.get(&CustomKey::Citations).unwrap();
        let citekeys = unwrap!(unreachable citekeys_value => String(s) => s);
        let citerefs = pandoc_cite(citekeys)?;
        storage.insert(CustomKey::Citations, Value::String(citerefs));
    }

    //println!("| cite step {:?}", step);
    match state {
        0 => {
            let cite_count = storage
                .get(&CustomKey::CiteCount)
                .map(|v| unwrap!(or_invalid v => Usize(x) => *x))
                .unwrap_or(Ok(0))?;
            storage.insert(CustomKey::CiteCount, Value::Usize(cite_count + 1));
            Ok((Dirty::Waiting, Value::Usize(cite_count)))
        }
        1 | 2 => {
            let list_value = storage.get_mut(&CustomKey::Citations).unwrap();
            let list = unwrap!(unreachable list_value => String(s) => s);
            list.push_str(match &args[0] {
                Value::Str(s) => s,
                Value::String(s) => s,
                _ => todo!(),
            });
            list.push('\n');
            list.push('\n');
            Ok((Dirty::Waiting, Value::Custom(CustomValue::Citation(id))))
        }
        3 | 4 => {
            let citerefs = storage.get_mut(&CustomKey::Citations).unwrap();
            let citerefs = unwrap!(unreachable citerefs => String(s) => s);
            let citation = citerefs.split("\n\n").nth(id).unwrap().to_string();
            Ok((Dirty::Ready, Value::String(citation)))
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
        .map(|v| unwrap!(unreachable v => Usize(x) => *x))
        .unwrap_or(0);
    match state {
        0 => Ok((Dirty::Waiting, Value::Str(""))),
        1 | 2 | 3 | 4 => {
            let cite_count = storage
                .get(&CustomKey::CiteCount)
                .map(|v| unwrap!(unreachable v => Usize(x) => *x))
                .unwrap_or(0);

            let citerefs = storage.get_mut(&CustomKey::Citations).unwrap();
            let citerefs = unwrap!(unreachable citerefs => String(s) => s);
            let ref_start = citerefs
                .split("\n\n")
                .skip(cite_count)
                .next()
                .unwrap()
                .as_ptr();
            let references = &citerefs[ref_start as usize - citerefs.as_ptr() as usize..];

            Ok((Dirty::Ready, Value::String(references.to_string())))
        }
        _ => unreachable!(),
    }
}

pub fn env<'a, V>(args: &[Value<'a, V>]) -> PureResult<'a, V> {
    let name: &str = match &args[0] {
        Value::Str(s) => s,
        Value::String(s) => s,
        _ => return Err("Invalid type, expecting string"),
    };
    fetch_env_var(name).map(Value::String)
}


////////////////////////////////////////////////////////////////////////////////

pub fn pandoc_cite(citekey: &str) -> Result<String, MyError> {
    let bibliography = fetch_env_var("BIBLIOGRAPHY")?;
    let citation = run_command(
        "pandoc",
        Some(citekey),
        //&["--citeproc", "-M", "suppress-bibliography=true", "-t", "plain",
        &["--citeproc", "-t", "plain", "--bibliography", &bibliography],
    )?;

    Ok(citation)
}

/******************************************************************************
 * Helpers
 ******************************************************************************/

pub fn run_command(program: &str, stdin: Option<&str>, args: &[&str]) -> Result<String, MyError> {
    let child = if let Some(s) = stdin {
        let mut child = process::Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(s.as_bytes()).unwrap();
        child
    } else {
        process::Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap()
    };
    let output = child.wait_with_output().unwrap();

    let out = String::from_utf8(output.stdout).unwrap();
    if output.status.success() {
        Ok(out)
    } else {
        panic!()
        //Err(CustomErr::NonZeroStatus(
        //    output.status.code().unwrap_or(1),
        //    out,
        //))
    }
}

fn fetch_env_var(key: &str) -> Result<String, MyError> {
    Ok(std::env::vars()
        .find(|(k, _)| k == key)
        .ok_or("Missing BIBLIOGRAPHY environment variable")?
        .1)
}
