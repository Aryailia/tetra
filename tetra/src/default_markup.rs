// This default flavour of this templating markup language. If you want
// to implement your own flavour (i.e. with your own functions), you should
// be able to copy this file directly.

//run: cargo test -- --nocapture

use std::borrow::Cow;
use std::fs;

use common::FileType;

// Do not use super so that if others want to make their own flavour, they
// can copy this file without issue
use crate::run::{Bindings, Dirty, Error, PureResult, StatefulResult};
use crate::run::{Value, Variables};

use crate::run::utility::{code, concat, env};
use crate::run::utility::{fetch_env_var, run_command};
use crate::run::value as v;
use crate::run::{LIMITED, UNLIMITED}; // these are just bools

use crate::api::Api;

// The main difference between pure and stateful functions is that
// * pure functions run only once (once all their arguments are ready) and
//   stateful functions until they report back that they are 'Dirty::Ready'
// * stateful functions gain access to a global namespace where they can store
//   data
//
// Registering is done by providing it
// * a name to be called while writing markup
// * reference to function definition
// * a enum (effectively a bool) that specifies whether to check the number
//   of arguments or not
// * a list for what types of arguments the function expects
pub fn default_context<'a>() -> Bindings<'a, CustomKey, CustomValue> {
    let mut ctx = Bindings::new();
    ctx.register_pure_function("env", &env, LIMITED, &[v::TEXT]);
    ctx.register_pure_function("include", &include, UNLIMITED, &[v::TEXT]);

    // "r/run <lang> <code-body>"
    ctx.register_pure_function("run", &code, LIMITED, &[v::TEXT, v::TEXT]);
    ctx.register_pure_function("r", &code, LIMITED, &[v::TEXT, v::TEXT]);
    ctx.register_pure_function(
        "if_equals",
        &if_eq_statement,
        LIMITED,
        &[v::TEXT, v::TEXT, v::TEXT],
    );
    ctx.register_pure_function(
        "run_if_equals",
        &run_if_equals,
        LIMITED,
        &[v::TEXT, v::TEXT, v::TEXT, v::TEXT],
    );
    ctx.register_pure_function(
        "run_env",
        &run_env,
        LIMITED,
        &[v::TEXT, v::TEXT, v::TEXT, v::TEXT],
    );

    ctx.register_pure_function("concat", &concat, UNLIMITED, &[]);
    ctx.register_pure_function("end", &concat, LIMITED, &[v::TEXT]);
    ctx.register_stateful_function("cite", &cite, LIMITED, &[v::TEXT]);
    ctx.register_stateful_function("references", &references, LIMITED, &[]);

    ctx.register_stateful_function("label_set", &label_set, LIMITED, &[v::TEXT, v::TEXT]);
    ctx.register_stateful_function("label", &label, LIMITED, &[v::TEXT]);
    ctx
}

////////////////////////////////////////////////////////////////////////////////
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CustomKey {
    Citations,
    CiteCount,
    CiteState,
    Label(String),
}

#[derive(Clone, Debug)]
pub enum CustomValue {
    CiteList(Vec<String>),
    Citation(usize),
}

////////////////////////////////////////////////////////////////////////////////

// The citations (e.g. parenthetical references) and references (bibliography
// listing full form citations) functions.

// Counts the citations on the first pass (for `Vec::with_capacity()`,
// accumulates all the citations on the second pass, pass that to pandoc
// and then prints out the citations
fn cite<'a>(
    args: &[Value<'a, CustomValue>],
    api: Api<'a>,
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
        let citerefs = pandoc_cite(citekeys, &api.meta.output_filetype)?;
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
            let list: &mut String =
                unwrap!(unreachable list_value => Value::Text(Cow::Owned(s)) => s);
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
    _api: Api<'a>,
    _: Value<'a, CustomValue>,
    storage: &mut Variables<'a, CustomKey, CustomValue>,
) -> StatefulResult<'a, CustomValue> {
    assert_eq!(0, args.len());

    let state = storage
        .get(&CustomKey::CiteState)
        .map(|v| unwrap!(unreachable v => Value::Usize(x) => *x))
        .unwrap_or(0);
    match state {
        0 | 1 | 2 => Ok((Dirty::Waiting, Value::Text(Cow::Borrowed("")))),
        3 | 4 => {
            let cite_count = storage
                .get(&CustomKey::CiteCount)
                .map(|v| unwrap!(unreachable v => Value::Usize(x) => *x))
                .unwrap_or(0);

            let citerefs = storage.get_mut(&CustomKey::Citations).unwrap();
            let citerefs = unwrap!(unreachable citerefs => Value::Text(s) => s);
            //println!("citerefs {}", citerefs);
            let ref_start = citerefs.split("\n\n").nth(cite_count).unwrap().as_ptr();
            let references = &citerefs[ref_start as usize - citerefs.as_ptr() as usize..];

            Ok((
                Dirty::Ready,
                Value::Text(Cow::Owned(references.to_string())),
            ))
        }
        _ => unreachable!(),
    }
}

pub fn pandoc_cite(citekey: &str, filetype: &FileType) -> Result<String, Error> {
    let bibliography = fetch_env_var("BIBLIOGRAPHY")?;
    let write_format = match filetype {
        FileType::AsciiDoctor => "asciidoctor",
        FileType::CommonMark => "commonmark",
        FileType::Markdown => "markdown_strict",
        FileType::RMarkdown => "markdown_strict",
        FileType::Pdf => panic!(),
        FileType::LaTeX => "latex",
        FileType::Html => "html5",
        FileType::Default => "plain",
    };
    let citation = run_command(
        "pandoc",
        Some(citekey),
        //&["--citeproc", "-M", "suppress-bibliography=true", "-t", "plain",
        &[
            "--citeproc",
            "-t",
            write_format,
            "--bibliography",
            &bibliography,
        ],
        None,
    )?;

    Ok(citation)
}

////////////////////////////////////////////////////////////////////////////////

// includes other files into the current file
// @TODO: add ability to parse those files as well
pub fn include<'a, V>(args: &[Value<'a, V>], _api: Api<'a>) -> PureResult<'a, V> {
    let path: &str = unwrap!(unreachable &args[0] => Value::Text(s) => s);
    let contents = fs::read_to_string(path).map_err(|err| {
        Error::Arg(
            0,
            Cow::Owned(format!("Could not read file {:?}: {}", path, err)),
        )
    })?;
    //let mut buffer = String::with_capacity(recursive_calc_length(args)?);
    //recursive_concat::<V>(args, &mut buffer);
    Ok(Value::Text(Cow::Owned(contents)))
}

////////////////////////////////////////////////////////////////////////////////

// includes other files into the current file
// @TODO: add ability to parse those files as well
pub fn if_eq_statement<'a, V>(args: &[Value<'a, V>], _api: Api<'a>) -> PureResult<'a, V> {
    let lvalue: &str = unwrap!(unreachable &args[0] => Value::Text(s) => s);
    let rvalue: &str = unwrap!(unreachable &args[1] => Value::Text(s) => s);
    if lvalue == rvalue {
        let contents: &str = unwrap!(unreachable &args[2] => Value::Text(s) => s);
        Ok(Value::Text(Cow::Owned(contents.to_string())))
    } else {
        Ok(Value::Text(Cow::Borrowed("")))
    }
}

pub fn run_if_equals<'a, V>(args: &[Value<'a, V>], api: Api<'a>) -> PureResult<'a, V> {
    let lvalue: &str = unwrap!(unreachable &args[0] => Value::Text(s) => s);
    let rvalue: &str = unwrap!(unreachable &args[1] => Value::Text(s) => s);
    if lvalue == rvalue {
        Ok(code(&args[2..], api)?)
    } else {
        Ok(Value::Text(Cow::Borrowed("")))
    }
}

////////////////////////////////////////////////////////////////////////////////

fn label_set<'a>(
    args: &[Value<'a, CustomValue>],
    _api: Api<'a>,
    _: Value<'a, CustomValue>,
    storage: &mut Variables<'a, CustomKey, CustomValue>,
) -> StatefulResult<'a, CustomValue> {
    let label_name: &str = unwrap!(unreachable &args[0] => Value::Text(s) => s);
    let label: Cow<str> = unwrap!(unreachable &args[1] => Value::Text(s) => s.clone());
    let label_key = CustomKey::Label(label_name.to_string());
    match storage.get(&label_key) {
        Some(_) => Err(Error::Arg(
            0,
            Cow::Owned(format!("The label {:?} has already been set", label_name)),
        )),
        None => {
            storage.insert(label_key, Value::Text(label.clone()));
            Ok((Dirty::Ready, Value::Text(label)))
        }
    }
}
fn label<'a>(
    args: &[Value<'a, CustomValue>],
    _api: Api<'a>,
    old_output: Value<'a, CustomValue>,
    storage: &mut Variables<'a, CustomKey, CustomValue>,
) -> StatefulResult<'a, CustomValue> {
    let label_name: &str = unwrap!(unreachable &args[0] => Value::Text(s) => s);
    let label = storage.get(&CustomKey::Label(label_name.to_string()));

    match (old_output, label)  {
        // If it has been set then return
        (_, Some(Value::Text(s))) => Ok((Dirty::Ready, Value::Text(s.clone()))),

        // First iteration
        (Value::Null, _) => Ok((Dirty::Waiting, Value::Usize(1))),

        // Any other iteration
        (Value::Usize(_), _) => Err(Error::Arg(
            0,
            Cow::Owned(format!("The label {:?} is not set. You must run `label_set({:?})` at some point in the document", label_name, label_name)),
        )),
        _ => unreachable!(),
    }
}

////////////////////////////////////////////////////////////////////////////////

// Same as `code()` but allows you set the environment variables
pub fn run_env<'a, V>(args: &[Value<'a, V>], _api: Api<'a>) -> PureResult<'a, V> {
    let id: &str = unwrap!(unreachable &args[0] => Value::Text(s) => s);
    let rvalue: &str = unwrap!(unreachable &args[1] => Value::Text(s) => s);
    let lang: &str = unwrap!(unreachable &args[2] => Value::Text(s) => s);
    let cell_body: &str = unwrap!(unreachable &args[3] => Value::Text(s) => s);

    match lang {
        "graphviz" | "dot" => {
            run_command("dot", Some(cell_body), &["-Tsvg"], Some(vec![(id, rvalue)]))
                .map(Cow::Owned)
                .map(Value::Text)
        }
        "sh" => run_command("sh", Some(cell_body), &["-s"], Some(vec![(id, rvalue)]))
            .map(Cow::Owned)
            .map(Value::Text),
        s => todo!("markup.rs: {}", s),
    }
}
