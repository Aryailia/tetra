//run: cargo test -- --nocapture

use crate::framework::{Source, Token};
use super::sexpr::{Arg, Sexpr};

pub type ParseOutput = (Vec<Command>, Vec<Token<Arg>>);
pub type ParseError = Token<&'static str>;

#[derive(Debug)]
pub struct Command {
    pub label: Label,
    pub args: (usize, usize),
}

pub fn process(sexprs: &[Sexpr], arg_defs: &[Token<Arg>]) -> Result<ParseOutput, ParseError> {
    //////////////////////////////////////////////////////////////////////////////
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

    //////////////////////////////////////////////////////////////////////////////
    // Reorder the arguments due to piping
    let mut resolved_args: Vec<Token<Arg>> = Vec::new();
    {
        // Because we are reordering, {buffer} holds values to be pushed after
        let mut buffer = Vec::new();

        for exp in &mut sorted_exprs {
            //println!("{}", exp.to_display(arg_defs, debug_source));
            let index = resolved_args.len();
            let unprocessed_args = &arg_defs[exp.args.0..exp.args.1];

            // Syntax check
            let mut start = 0;
            let output_id = stdin_refs[exp.cell_id / 2];

            // Handle the re-ordering:
            // The possibilities are:
            // - (value/ident, pipe, ...) three or more arguments
            // - (pipedstdin) or (pipestdin, ...) one or more arguments
            // - (ident, =, value/ident)
            //
            // After re-arranging the argument order, the possibilities are:
            // - (no idents, no idents, ...) one or more arguments
            // - (ident, anything, anything)
            // - (=, ident, value/ident)
            for (i, arg) in unprocessed_args.iter().enumerate().rev() {
                match (i, &arg.me) {
                    //(0, 1) => {}
                    // Move "a | b" to "b a"
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
                    (_, Arg::Pipe) => return Err(Token::new("ast.rs: Missing a value before the pipe to pass to the next command.", arg.source.clone())),
                    (0, Arg::PipedStdin) => {
                        buffer.push(Token::new(
                            Arg::Reference(output_id),
                            unprocessed_args[0].source.clone(),
                        ));
                        start = 1;
                    }
                    (_, Arg::PipedStdin) => unreachable!("{}", file!()),
                    (0, Arg::Assign) => {
                        // Arg::Assign is already in the label location and not
                        // in order as is all the other sexpr tokens
                        if unprocessed_args.len() != 3 {
                            if unprocessed_args.len() < 3 {
                                return Err(Token::new("ast.rs: This assign is missing an r-value", arg.source.clone()));
                            } else {
                                return Err(Token::new("ast.rs: Unexpected second argument to for assign", buffer[3].source.clone()));
                            }
                        } else if !matches!(unprocessed_args[1].me, Arg::Ident) {
                            return Err(Token::new("ast.rs: The l-value of the assign should be an ident", buffer[1].source.clone()));
                        }
                    }
                    (_, Arg::Assign) => return Err(Token::new("ast.rs: There correct syntax for variable assignments is:\n    '<l-value> = <r-value>'", arg.source.clone())),

                    _ => {}
                }
            }

            // Exhaustively enumerate so that we update this if we change
            // how the sexpr parser works
            resolved_args.extend(
                unprocessed_args[start..]
                    .iter()
                    //.map(|arg| {
                    //    println!("{:?}", arg.to_display(debug_source));
                    //    arg
                    //})
                    .filter_map(|arg| match arg.me {
                        Arg::Pipe => unreachable!(),
                        Arg::PipedStdin => unreachable!(),
                        Arg::Stdin => {
                            Some(Token::new(Arg::Reference(output_id), arg.source.clone()))
                        }
                        Arg::Assign => Some(arg.clone()),
                        Arg::Str | Arg::Char(_) | Arg::Reference(_) => Some(arg.clone()),
                        Arg::Ident | Arg::IdentFunc => Some(arg.clone()),
                    })
            );
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
    // Parse {resolved_args} and {sorted_exprs} into a Vec<Command> and {resolved_args}
    let mut output = Vec::with_capacity(sorted_exprs.len());
    output.extend(sorted_exprs.iter().map(|exp| {
        let len = exp.args.1 - exp.args.0;
        let (label, skip) = (len > 0)
            .then(|| {
                let first_arg = &resolved_args[exp.args.0];
                match first_arg.me {
                    Arg::Ident | Arg::IdentFunc => (Label::Ident(first_arg.source.clone()), 1),
                    Arg::Assign => (Label::Assign(first_arg.source.clone()), 1),
                    _ => (Label::Display, 0),
                }
            })
            .unwrap_or((Label::Display, 0));
        Command {
            label,
            args: (exp.args.0 + skip, exp.args.1),
        }
    }));

    // @TODO: Trim the {resolved_args} via retain?
    Ok((output, resolved_args))
    //Err(Token::new("Finished parsing", Source::Range(0, 0)))
}

#[derive(Debug)]
pub enum Label {
    Assign(Source), // "<l-value> = <r-value>"
    Display,        // Just display all the arguments as is
    Ident(Source),  //
}

impl Command {
    //#[Config(debug)]
    pub fn to_display(&self, args: &[Token<Arg>], source: &str) -> String {
        let mut display = String::new();
        display.push_str(match &self.label {
            Label::Assign(_) => "=",
            Label::Display => "Display",
            Label::Ident(s) => s.to_str(source),
        });
        display.push('(');

        for arg in &args[self.args.0..self.args.1] {
            display.push_str(&format!("{}, ", arg.to_display(source)));
        }
        display.push(')');
        display
    }
}
