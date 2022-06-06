//run: cargo test -- --nocapture

// Second pass over the s-exprs to format them into runnable commands
// Removes the superfluous redirections, resolves what Stdin references,
// and removes gaps.

use super::sexpr::{Arg, Sexpr};
use crate::framework::{Source, Token};

pub type ParseOutput = (Vec<Command>, Vec<Token<Arg>>, Vec<usize>);
pub type ParseError = Token<&'static str>;

#[derive(Debug)]
pub struct Command {
    pub label: Label,
    pub args: (usize, usize),
    pub provides_for: (usize, usize),
}

pub fn process(sexprs: &[Sexpr], arg_defs: &[Token<Arg>]) -> Result<ParseOutput, ParseError> {
    ////////////////////////////////////////////////////////////////////////////
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

    ////////////////////////////////////////////////////////////////////////////
    // Resolve 'Arg::Stdin' to a 'Arg::Reference(_)' and syntax check
    let mut resolved_args = Vec::with_capacity(arg_defs.len());
    for exp in &mut sorted_exprs {
        let output_id = stdin_refs[exp.cell_id / 2];

        let start = resolved_args.len();
        let parameters = &arg_defs[exp.args.0..exp.args.1];
        for (i, arg) in parameters.iter().enumerate() {
            resolved_args.push(match arg.me {
                Arg::Stdin => arg.remap(Arg::Reference(output_id)),
                // These branches made impossible by sexpr.rs parse step
                Arg::Assign | Arg::IdentFunc if i >= 1 => unreachable!(),
                Arg::Ident if i >= 2 => unreachable!(),
                _ => arg.clone(),
            });
            if i == 1 && matches!(arg.me, Arg::Ident) {
                debug_assert_eq!(parameters[0].me, Arg::Assign);
            }
        }
        exp.args = (start, resolved_args.len());
    }

    ////////////////////////////////////////////////////////////////////////////
    // Optimisation step, remove any single command
    // @TODO: change this to not be O(n^2) if possible
    sorted_exprs.retain(|exp| {
        let first_index = exp.args.0;
        let len = exp.args.1 - first_index;
        if len == 1 {
            match resolved_args[first_index].me {
                // Replace a pointer to a literal with just the literal
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

                // Replace double pointers with a direct pointer
                // e.g. `{1} -> {2} -> {3}` replaced with `{1} -> {3}`
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

    ////////////////////////////////////////////////////////////////////////////
    // Parse {resolved_args} and {sorted_exprs} into a Vec<Command> and {resolved_args}
    //
    // Map ids of the output of each s-expr to their indices in {sorted_exprs}
    // for use in changing 'Arg::Reference(<id>)' to 'Arg::Reference(<index>)'
    // in the final loop
    let mut output_indices = vec![0; sexprs.len()];
    for (i, exp) in sorted_exprs.iter().enumerate() {
        output_indices[exp.out] = i;
    }

    // Build final result array
    let mut output = Vec::with_capacity(sorted_exprs.len());
    let mut gapless_args = Vec::with_capacity(arg_defs.len());
    let mut dependencies = Vec::with_capacity(arg_defs.len());
    for (i, exp) in sorted_exprs.iter().enumerate() {
        let len = exp.args.1 - exp.args.0;

        // Discriminate label from parameters
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

        // Build {gapless_args} by removing the gaps in {resolved_args}
        let new_start = gapless_args.len();
        for a in &resolved_args[exp.args.0 + skip..exp.args.1] {
            match a.me {
                // Change from 'Reference(<id>)' to 'Reference(<index into {output}>)'
                Arg::Reference(j) => {
                    let index = output_indices[j];
                    gapless_args.push(a.remap(Arg::Reference(index)));
                    dependencies.push((index, i));
                }
                _ => gapless_args.push(a.clone()),
            }
        }
        output.push(Command {
            label,
            args: (new_start, gapless_args.len()),
            provides_for: (0, 0),
        })
    }

    // O(n log n) determine what each command provides for
    // Thus we know the dependencies (the args) and reverse dependencies of
    // all commands
    dependencies.sort_unstable();
    let providees = dependencies.iter().map(|x| x.1).collect::<Vec<_>>();

    let mut cursor = 0;
    let mut last_provider = dependencies[0].0;
    for (i, (provider, _)) in dependencies.iter().enumerate().skip(1) {
        if *provider != last_provider {
            output[last_provider].provides_for = (cursor, i);
            last_provider = *provider;
            cursor = i;
        }
    }

    Ok((output, gapless_args, providees))
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
            Label::Display => "Concat",
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
