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
    let mut sorted_exprs: Vec<Sexpr> = Vec::with_capacity(sexprs.len());
    let stdin_refs = {
        let cell_count = sexprs.last().unwrap().cell_id / 2;
        let mut stdin_refs = Vec::with_capacity(cell_count + 1);

        // Move the evens after the odds, we start at 0
        let mut buffer: Vec<Sexpr> = Vec::with_capacity(sexprs.len());
        let mut past_id = 0;
        for exp in sexprs {
            if past_id + 2 <= exp.cell_id {
                past_id += 2;
                bound_push!(stdin_refs, sorted_exprs[sorted_exprs.len() - 1].out);
                sorted_exprs.append(&mut buffer);
            }
            if exp.cell_id % 2 == 0 {
                bound_push!(buffer, exp.clone());
            } else {
                bound_push!(sorted_exprs, exp.clone());
            }
        }
        sorted_exprs.append(&mut buffer); // Add the final knit command

        // So that 'stdin_refs[x]' for the knit command does not go out of bounds
        bound_push!(stdin_refs, 0);
        stdin_refs
    };

    ////////////////////////////////////////////////////////////////////////////
    // Resolve 'Arg::Stdin' to a 'Arg::Reference(_)' and syntax check
    let mut resolved_args = Vec::with_capacity(arg_defs.len());
    for exp in &mut sorted_exprs {
        let output_id = stdin_refs[exp.cell_id / 2];

        let start = resolved_args.len();
        let parameters = &arg_defs[exp.args.0..exp.args.1];
        for (i, arg) in parameters.iter().enumerate() {
            bound_push!(
                resolved_args,
                match arg.me {
                    Arg::Stdin => arg.remap(Arg::Reference(output_id)),
                    // These branches made impossible by sexpr.rs parse step
                    Arg::Assign | Arg::IdentFunc if i >= 1 => unreachable!(),
                    Arg::Ident if i >= 2 => unreachable!(),
                    _ => arg.clone(),
                }
            );

            if i == 1 && matches!(arg.me, Arg::Ident) {
                debug_assert_eq!(parameters[0].me, Arg::Assign);
            }
        }
        exp.args = (start, resolved_args.len());
    }
    //sorted_exprs.iter().for_each(|s| println!("asdf {:?}", s));
    //sorted_exprs.iter().for_each(|exp|
    //    println!(
    //        "asdf {:?} {:?}",
    //        exp,
    //        &resolved_args[exp.args.0 .. exp.args.1],
    //    ));

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
                    let (first, rest) = resolved_args[first_index..].split_at_mut(1);
                    let first_arg = &first[0];
                    rest.iter_mut().for_each(|arg| {
                        if let Arg::Reference(i) = arg.me {
                            if i == exp.out {
                                *arg = first_arg.clone();
                            }
                        }
                    });
                    // Do not remove if there are no more arguments left
                    rest.is_empty()
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
    // {dependencies} is Vec<(usize, usize)> which means data flows from usize1
    // to usize2, i.e. {output[usize2]} uses {output[usize1]} as an argument
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
                    bound_push!(gapless_args, a.remap(Arg::Reference(index)));
                    bound_push!(dependencies, (index, i));
                }
                _ => bound_push!(gapless_args, a.clone()),
            }
        }
        bound_push!(
            output,
            Command {
                label,
                args: (new_start, gapless_args.len()),
                provides_for: (0, 0),
            }
        )
    }

    ////////////////////////////////////////////////////////////////////////////
    // Set {output[].provides_for} to demarcate reverse dependent groups

    // O(n log n) determine what each command provides for
    // Thus we know the dependencies (the args) and reverse dependencies of
    // all commands
    dependencies.sort_unstable();

    // {dependencies} is sorted by the second value. We essentially are
    // instead sorting by the right values.
    //
    // Like output {output[].args} is a range into {gapless_args},
    // {output[].provides_for} is a range into {providees}
    let mut cursor = 0; // cursor for ranges in {providees}
    let mut last_provider = dependencies.len(); // Used to determine groupings

    // {last_provider} at {dependencies.len()} is guaranteed to trigger at
    // {i} = 1
    for (i, (provider, _)) in dependencies.iter().enumerate() {
        if *provider != last_provider {
            last_provider = *provider;
            cursor = i;
        }
        output[last_provider].provides_for = (cursor, i + 1);
    }

    // Some sanity checks in debug mode
    #[cfg(debug_assertions)]
    {
        //println!("{:?}", dependencies);
        //for (i, cmd) in output.iter().enumerate() {
        //    let range = &dependencies[cmd.provides_for.0..cmd.provides_for.1];
        //    println!("{} {:?}", i, range);
        //}

        // Ensure these are valid ranges
        // One concern is {last_provider} init to 0 is fine
        for cmd in &output {
            debug_assert!(cmd.provides_for.0 <= cmd.provides_for.1);
        }

        // Ensure {output[].provides_for} is demarcating the right commands
        // (injective check)
        for (i, cmd) in output.iter().enumerate() {
            let range = &dependencies[cmd.provides_for.0..cmd.provides_for.1];
            for (provider, _receiver) in range {
                debug_assert_eq!(i, *provider);
            }
        }

        // Make sure all reverse dependencies map somewhere (surjective check)
        let count = output
            .iter()
            .map(|cmd| cmd.provides_for.1 - cmd.provides_for.0)
            .sum();
        debug_assert_eq!(dependencies.len(), count);
    }

    // Remove the left values from {dependencies}
    let providees = dependencies.iter().map(|x| x.1).collect::<Vec<_>>();

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
