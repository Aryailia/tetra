//run: cargo test -- --nocapture

use crate::framework::{Source, Token};
use crate::sexpr::{Arg, Sexpr};

pub type ParseOutput = (Vec<Command>, Vec<Token<Arg>>);
pub type ParseError = Token<&'static str>;

#[derive(Debug)]
pub struct Command {
    pub label: Option<Source>,
    pub args: (usize, usize),
}

pub fn process(
    sexprs: &[Sexpr],
    arg_defs: &[Token<Arg>],
    debug_source: &str,
) -> Result<ParseOutput, ParseError> {
    // Reorder so that the HereDoc headers appear after their bodies
    let mut sorted_exprs: Vec<Sexpr> = Vec::new();
    let mut stdin_refs: Vec<usize> = Vec::new();
    {
        // Move the evens after the odds, we start at 0
        let mut buffer: Vec<Sexpr> = Vec::new();
        let mut past_id = 0;
        for exp in sexprs {
            if past_id + 2 <= exp.cell_id {
                past_id += 2;
                stdin_refs.push(sorted_exprs[sorted_exprs.len() - 1].out);
                sorted_exprs.append(&mut buffer);
            }
            if exp.cell_id % 2 == 0 {
                buffer.push(exp.clone());
            } else {
                sorted_exprs.push(exp.clone());
            }
        }
        sorted_exprs.append(&mut buffer); // Add the final knit command

        // So that 'stdin_refs[x]' for the knit command does not go out of bounds
        stdin_refs.push(0);
    }

    // Reorder the arguments due to piping
    let mut resolved_args: Vec<Token<Arg>> = Vec::new();
    {
        let mut buffer = Vec::new();
        for exp in &mut sorted_exprs {
            //println!("{}", exp.to_display(arg_defs, debug_source));
            let index = resolved_args.len();
            let unprocessed_args = &arg_defs[exp.args.0..exp.args.1];

            // Syntax check
            let mut start = 0;
            let output_id = stdin_refs[exp.cell_id / 2];

            // The possibilities are:
            // - (value/ident, pipe, ...) three or more arguments
            // - (pipedstdin) or (pipestdin, ...) one or more arguments
            //
            // After re-arranging the argument order, the possibilities are:
            // - (no idents, no idents, ...) one or more arguments
            // - (ident, anything, anything)
            for (i, arg) in unprocessed_args.iter().enumerate().rev() {
                match (i, &arg.me) {
                    //(0, 1) => {}
                    (1, Arg::Pipe) => {
                        if unprocessed_args.len() < 3 {
                            let pipe = unprocessed_args
                                .iter()
                                .find(|a| matches!(a.me, Arg::Pipe))
                                .unwrap();
                            return Err(Token::new("blah", pipe.source.clone()));
                        } else {
                            buffer.push(unprocessed_args[0].clone());
                            start = 2;
                        }
                    }
                    (_, Arg::Pipe) => return Err(Token::new("hello", arg.source.clone())),
                    (0, Arg::PipedStdin) => {
                        buffer.push(Token::new(
                            Arg::Reference(output_id),
                            unprocessed_args[0].source.clone(),
                        ));
                        start = 1;
                    }
                    (_, Arg::PipedStdin) => return Err(Token::new("asdf", arg.source.clone())),
                    _ => {}
                }
            }

            // Exhaustively enumerate so that we update this if we change
            // how the sexpr parser works
            resolved_args.extend(unprocessed_args[start..].iter().map(|arg| match arg.me {
                Arg::Pipe => unreachable!(),
                Arg::PipedStdin => unreachable!(),
                Arg::Stdin => Token::new(Arg::Reference(output_id), arg.source.clone()),
                Arg::Str | Arg::Char(_) | Arg::Reference(_) => arg.clone(),
                Arg::Unknown => arg.clone(),
            }));
            resolved_args.append(&mut buffer);
            exp.args = (index, resolved_args.len());
        }
    }

    //////////////////////////////////////////////////////////////////////////////
    //// Optimisation step, remove any single command
    // @TODO: change this to not be O(n^2) if possible

    // Map ids of the output of each s-expr to their indicies in {sorted_exprs}
    //let mut output_indices = vec![0; sorted_exprs[sorted_exprs.len() - 1].out + 1];
    //for (i, exp) in sorted_exprs.iter().enumerate() {
    //    output_indices[exp.out] = i;
    //}

    //let mut output = Vec::with_capacity(sorted_exprs.len());
    {
        sorted_exprs.retain(|exp| {
            let first_index = exp.args.0;
            let len = exp.args.1 - first_index;
            if len == 1 {
                match resolved_args[first_index].me {
                    // If a literal, replace all Arg::Reference with the literal
                    Arg::Str | Arg::Char(_) => {
                        let (first, args) = resolved_args[first_index..].split_at_mut(1);
                        let first_arg = &first[0];
                        args.iter_mut().for_each(|arg| {
                            if let Arg::Reference(i) = arg.me {
                                if i == exp.out {
                                    *arg = first_arg.clone();
                                }
                            }
                        });
                        false
                    }
                    Arg::Reference(old_i) => {
                        resolved_args[first_index + 1..].iter_mut().for_each(|arg| {
                            if let Arg::Reference(i) = arg.me {
                                if i == exp.out {
                                    arg.me = Arg::Reference(old_i);
                                }
                            }
                        });
                        false
                    }
                    _ => true,
                }
            } else {
                true
            }
        });
    }

    ////////////////////////////////////////////////////////////////////////////
    // Map ids of the output of each s-expr to their indices in {sorted_exprs}
    let mut output_indices = vec![0; sorted_exprs[sorted_exprs.len() - 1].out + 1];
    for (i, exp) in sorted_exprs.iter_mut().enumerate() {
        output_indices[exp.out] = i;
        exp.out = i; // Do not need {Sexp.out} anymore as everything is sorted
    }
    // And remap all the {Arg::Reference()}s to their new indices
    for arg in resolved_args.iter_mut() {
        if let Arg::Reference(i) = arg.me {
            arg.me = Arg::Reference(output_indices[i]);
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    let mut output = Vec::with_capacity(sorted_exprs.len());
    output.extend(sorted_exprs.iter().map(|exp| {
        let len = exp.args.1 - exp.args.0;
        let label = (len > 0)
            .then(|| &resolved_args[exp.args.0])
            .and_then(|first_arg| match first_arg.me {
                Arg::Unknown => Some(first_arg.source.clone()),
                _ => None,
            });
        if label.is_some() {
            Command {
                label,
                args: (exp.args.0 + 1, exp.args.1),
            }
        } else {
            Command {
                label,
                args: (exp.args.0, exp.args.1),
            }
        }
    }));

    // @TODO: Trim the {resolved_args} via retain?

    //for exp in &sorted_exprs {
    //    println!("{}", exp.to_display(&resolved_args, debug_source));
    //}
    //for (i, exp) in output.iter().enumerate() {
    //    println!("{} -> {}", exp.to_display(&resolved_args, debug_source), i);
    //}

    //Ok((sorted_exprs, resolved_args))
    Ok((output, resolved_args))
    //Err(Token::new("Finished parsing", Source::Range(0, 0)))
}

impl Command {
    //#[Config(debug)]
    pub fn to_display(&self, args: &[Token<Arg>], source: &str) -> String {
        let mut display = String::new();
        display.push_str(&format!(
            "{}(",
            self.label
                .as_ref()
                .map(|s| s.to_str(source))
                .unwrap_or("Display")
        ));
        for arg in &args[self.args.0..self.args.1] {
            display.push_str(&format!("{}, ", arg.to_display(source)));
        }
        display.push_str(")");
        display
    }
}
