// This is the set of functions that probably all custom flavours of this
// templating markup language can make use of. The bare bones of this language
// is this file and the executor.rs

//run: cargo test -- --nocapture

use std::borrow::{Borrow, Cow};
use std::io::Write;
use std::process;
use std::process::Stdio;

use super::{Error, PureResult, Value};
use crate::api::Api;

/******************************************************************************
 * In-built Commands
 ******************************************************************************/

////////////////////////////////////////////////////////////////////////////////
// Concat

// Just joins its arguments into a string
// Also doubles as the default push to the final knit
pub fn concat<'a, V>(args: &[Value<'a, V>], _api: Api<'a>) -> PureResult<'a, V> {
    let mut buffer = String::with_capacity(recursive_calc_length(args)?);
    recursive_concat::<V>(args, &mut buffer);
    Ok(Value::Text(Cow::Owned(buffer)))
}

fn recursive_calc_length<V>(args: &[Value<V>]) -> Result<usize, Error> {
    let mut sum = 0;
    for (i, a) in args.iter().enumerate() {
        sum += match a {
            Value::Null => return Err(Error::Arg(i, "You left a null unprocessed".into())),
            Value::Text(s) => s.len(),
            Value::Char(c) => c.len_utf8(),
            Value::Usize(x) => x.to_string().len(),
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
            Value::Text(s) => buffer.push_str(s),
            Value::Char(c) => buffer.push(*c),
            Value::Usize(x) => buffer.push_str(&x.to_string()),
            Value::Bool(b) => buffer.push_str(b.then(|| "true").unwrap_or("false")),
            Value::List(l) => recursive_concat(l, buffer),
            Value::Custom(_) => todo!(),
        };
    }
}

////////////////////////////////////////////////////////////////////////////////
// shell
pub fn shell<'a, V>(args: &[Value<'a, V>], _api: Api<'a>) -> PureResult<'a, V> {
    //let lang = unwrap!(or_invalid args[0] => Value::Text(x) => x);
    let cmd: &str = match &args[0] {
        Value::Text(x) => x,
        _ => return Err(Error::Arg(0, "Invalid type".into())),
    };
    let last_index = args.len() - 1;
    if last_index == 0 {
        Err(Error::Generic(Cow::Borrowed("Missing second argument that will be used for STDIN")))
    } else {
        let cell_body: &str = match &args[last_index] {
            Value::Text(x) => x,
            _ => return Err(Error::Arg(last_index, "Invalid type".into())),
        };
        let mut args: Vec<&str> = args[1..last_index]
            .iter()
            .enumerate()
            .map(|(i, a)| match a{
                Value::Text(x) => Ok(x.borrow()),
                _ => return Err(Error::Arg(i, "Invalid type. Expected text.".into())),
            })
            .collect::<Result<Vec<&str>, Error>>()?;
        run_command(cmd, Some(cell_body), &args, None)
            .map(Cow::Owned)
            .map(Value::Text)
    }
}

////////////////////////////////////////////////////////////////////////////////
// env
pub fn env<'a, V>(args: &[Value<'a, V>], _api: Api<'a>) -> PureResult<'a, V> {
    let name: &str = match &args[0] {
        Value::Text(s) => s,
        _ => return Err(Error::Arg(0, "Invalid type, expecting string".into())),
    };
    fetch_env_var(name).map(Cow::Owned).map(Value::Text)
}

/******************************************************************************
 * Helpers
 ******************************************************************************/

pub fn run_command(
    program: &str,
    stdin: Option<&str>,
    args: &[&str],
    env: Option<Vec<(&str, &str)>>,
) -> Result<String, Error> {
    let mut process = process::Command::new(program);
    process.args(args);
    if let Some(e) = env {
        process.envs(e);
    }

    let child = if let Some(s) = stdin {
        let mut child = process
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(s.as_bytes()).unwrap();
        child
    } else {
        process
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap()
    };
    let output = child.wait_with_output().unwrap();

    if output.status.success() {
        println!("{:?}", String::from_utf8(output.stderr));
        Ok(String::from_utf8(output.stdout).unwrap())
    } else {
        Err(Error::Generic(Cow::Owned(format!("Non zero status: {}\n{}", "?", String::from_utf8(output.stderr).unwrap()))))
        //panic!("NonZeroStatus {}", )
        //Err(CustomErr::NonZeroStatus(
        //    output.status.code().unwrap_or(1),
        //    out,
        //))
    }
}

pub fn fetch_env_var(key: &str) -> Result<String, Error> {
    Ok(std::env::vars()
        .find(|(k, _)| k == key)
        .ok_or_else(|| Error::Generic(Cow::Owned(format!("Missing {} environment variable", key))))?
        .1)
}
