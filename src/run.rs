//run: cargo test -- --nocapture
macro_rules! unwrap {
    (or_invalid $value:expr => $type:ident($x:ident) => $output:expr) => {
        match $value {
            Value::$type($x) => Ok($output),
            _ => Err("Invalid type"),
        }
    };
    (unreachable $value:expr => $type:ident($x:ident) => $output:expr) => {
        match $value {
            Value::$type($x) => $output,
            _ => unreachable!(),
        }
    };
}


pub mod executor;
pub mod markup;

