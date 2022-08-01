//run: cargo test -- --nocapture

// Second pass over the s-exprs to format them into runnable commands
// Removes the superfluous redirections, resolves what Stdin references,
// and removes gaps.

use super::sexpr::Sexpr;
use super::{Item, Label, Param, SexprOutput};
use crate::framework::Token;

pub struct AstOutput(pub Vec<Command>, pub Vec<Token<Param>>, pub Vec<usize>);
pub type ParseError = Token<&'static str>;

#[derive(Debug)]
pub struct Command {
    pub label: Token<Label>,
    pub args: (usize, usize),
    pub provides_for: (usize, usize),
}

pub fn process(sexprs_output: &SexprOutput, _debug_source: &str) -> Result<AstOutput, ParseError> {
    let (s, resolved_params) = resolve_stdin_and_optimise(sexprs_output);
    //s
    //    .iter()
    //    .enumerate()
    //    .for_each(|(i, s)| println!("{:<3} {}", i, s.to_display2(&resolved_params, _debug_source)));

    // Trim args and realign the references
    let ast = trim_and_remap_parameters(sexprs_output.0.len(), s, resolved_params)?;
    //ast.0.iter().enumerate().for_each(|(i, t)| {
    //    println!(
    //        "{:?} | {} -> {}",
    //        &ast.2[t.provides_for.0..t.provides_for.1],
    //        t.to_display(&ast.1, _debug_source),
    //        i
    //    )
    //});

    Ok(ast)
}

fn resolve_stdin_and_optimise(
    SexprOutput(sexprs, items): &SexprOutput,
) -> (Vec<Sexpr>, Vec<Token<Param>>) {
    ////////////////////////////////////////////////////////////////////////////
    // Reorder so that the HereDoc headers appear after their bodies
    // i.e. from order
    //     head 1 body 2 head 3 body 4
    // swap to
    //     body 2 head 1 body 4 head 3
    // this is topologically sorted order.
    //
    // More specifically, we only want to move header commands that use
    // 'Item::Stdin' since it will be resolved to a 'Param::Reference' and
    // will be the only ones that reference forward.
    debug_assert!(!sexprs.is_empty());
    let sexpr_count = sexprs.len();

    // Topological sort and find the {output_id} for the text cells' concat
    let mut sorted_sexpr_indices = Vec::with_capacity(sexpr_count);
    let stdin_refs = {
        debug_assert_eq!(sexprs[sexpr_count - 1].cell_id % 2, 0);
        let cell_count = sexprs[sexpr_count - 1].cell_id / 2;
        let mut buffer = Vec::with_capacity(sexpr_count);

        // + 1 for the 0 pushed at the end
        let mut stdin_refs = Vec::with_capacity(cell_count + 1);
        let mut last_body_output_id = 0; // The output_id of the last text cell
        let mut last_parity = 0;

        for (i, exp) in sexprs.iter().enumerate() {
            let curr_parity = exp.cell_id % 2;
            if curr_parity == 1 {
                last_body_output_id = i;
                // pushing body commands
                bound_push!(sorted_sexpr_indices, i);
            } else {
                // On change from odd {exp.cell_id} to even {exp.cell_id}
                if curr_parity != last_parity {
                    bound_push!(stdin_refs, last_body_output_id);
                    // Push all commands that depend on the text body
                    // into sorted list now that all the body commands have
                    // been pushed
                    sorted_sexpr_indices.append(&mut buffer);
                }

                // Push any entries with 'Item::Stdin' into {buffer}
                let parameters = &items[exp.args.0..exp.args.1];
                if parameters.iter().any(|t| matches!(t.me, Item::Stdin)) {
                    bound_push!(buffer, i);
                } else {
                    bound_push!(sorted_sexpr_indices, i);
                }
            }
            last_parity = exp.cell_id % 2
        }

        debug_assert!(buffer.is_empty()); // Make sure all appends went through

        // So that 'stdin_refs[x]' for the knit command does not go out of bounds
        bound_push!(stdin_refs, 0);
        stdin_refs
    };

    ////////////////////////////////////////////////////////////////////////////
    // Resolve 'Item::Stdin' -> 'Param::Reference'
    // Also trim out:
    // 1) 'Reference(to Param::basic-type)' to just the 'Param::basic-type'
    // 2) Double pointers to a direct pointer,
    //    e.g. '{1} -> {2} -> {3}' to '{1} -> {3}'

    // Used to know which pointers are no longer used due to optimisation 2).
    let mut sexpr_times_referenced = vec![0; sexpr_count];
    let mut resolved_params = Vec::with_capacity(items.len());
    for exp in sexprs {
        for (i, item) in items[exp.args.0..exp.args.1].iter().enumerate() {
            let param = match item.me {
                Item::Reference(_) | Item::Stdin => {
                    let id = match item.me {
                        Item::Stdin => stdin_refs[exp.cell_id / 2],
                        Item::Reference(i) => i,
                        _ => unreachable!(),
                    };

                    let target = &sexprs[id];
                    if matches!(target.head.me, Label::Concat) && target.args.1 - target.args.0 == 1
                    {
                        let target_as_item = &items[sexprs[id].args.0];
                        match target_as_item.me {
                            Item::Str => target_as_item.remap(Param::Str),
                            Item::Literal(s) => target_as_item.remap(Param::Literal(s)),

                            // Replace double pointers with a direct pointer
                            Item::Reference(real_id) => {
                                sexpr_times_referenced[real_id] += 1;
                                item.remap(Param::Reference(real_id))
                            }
                            _ => {
                                sexpr_times_referenced[id] += 1;
                                item.remap(Param::Reference(id))
                            }
                        }
                    } else {
                        sexpr_times_referenced[id] += 1;
                        item.remap(Param::Reference(id))
                    }
                }

                // The rest is just one-to-one mapping from {Item} to {Param}
                Item::Ident if i >= 1 => unreachable!(
                    "\n{:?}\nThese should all be Item::Reference()\n",
                    exp.to_debug(items)
                ),

                Item::Str => item.remap(Param::Str),
                Item::Literal(s) => item.remap(Param::Literal(s)),
                Item::Ident => item.remap(Param::Ident),

                // These branches made impossible by sexpr.rs parse step
                Item::Func
                | Item::Pipe
                | Item::PipedStdin
                | Item::Assign
                | Item::Concat
                | Item::Comma
                | Item::Paren
                | Item::Stmt => unreachable!(),
            };
            bound_push!(resolved_params, param);
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Optimisation step
    let mut trimmed_sexprs = Vec::with_capacity(sexpr_count);
    for i in sorted_sexpr_indices.drain(..) {
        let exp = &sexprs[i];

        let start = exp.args.0;
        let close = exp.args.1;
        if close - start == 1
            && matches!(&exp.head.me, Label::Concat)
            && sexpr_times_referenced[i] == 0
        {
            match resolved_params[start].me {
                Param::Str | Param::Literal(_) | Param::Reference(_) => {}
                _ => bound_push!(trimmed_sexprs, exp.clone()),
            }
        } else {
            bound_push!(trimmed_sexprs, exp.clone());

            //bound_push!(trimmed_sexprs, (
            //        exp.output_id,
            //        Command {
            //            label: exp.head.clone(),
            //            args: exp.args.clone(),
            //            provides_for: (0, 0),
            //        }
            //));
        }
    }

    //println!("{:?}", sexpr_times_referenced);
    (trimmed_sexprs, resolved_params)
}

fn trim_and_remap_parameters(
    sexpr_count: usize,
    mut trimmed_cmds: Vec<Sexpr>,
    resolved_args: Vec<Token<Param>>,
) -> Result<AstOutput, ParseError> {
    let item_count = resolved_args.len();
    ////////////////////////////////////////////////////////////////////////////
    // Parse {resolved_args} and {trimmed_cmds} into a Vec<Command> and {resolved_args}
    //
    // Map ids of the output of each s-expr to their indices in {trimmed_cmds}
    // for use in changing 'Param::Reference(<id>)' to 'Param::Reference(<index>)'
    // in the final loop
    let mut output_indices = vec![0; sexpr_count];
    for (i, exp) in trimmed_cmds.iter().enumerate() {
        output_indices[exp.output_id] = i;
    }

    // Build final result array
    // {dependencies} is Vec<(usize, usize)> which means data flows from usize1
    // to usize2, i.e. {output[usize2]} uses {output[usize1]} as an argument
    let mut output = Vec::with_capacity(trimmed_cmds.len());
    let mut gapless_args = Vec::with_capacity(item_count);
    let mut dependencies = Vec::with_capacity(item_count);
    for (i, exp) in trimmed_cmds.drain(..).enumerate() {
        // Discriminate label from parameters
        let label = exp.head;

        // Build {gapless_args} by removing the gaps in {resolved_args}
        let new_start = gapless_args.len();
        // @TODO: replace this with a drain
        for a in &resolved_args[exp.args.0..exp.args.1] {
            match a.me {
                // Change from 'Reference(<id>)' to 'Reference(<index into {output}>)'
                Param::Reference(j) => {
                    let index = output_indices[j];
                    bound_push!(gapless_args, a.remap(Param::Reference(index)));
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

    Ok(AstOutput(output, gapless_args, providees))
}

impl Command {
    pub fn to_display(&self, args: &[Token<Param>], source: &str) -> String {
        let mut display = String::new();
        self.label.push_display(&mut display, source);
        display.push('[');

        for arg in &args[self.args.0..self.args.1] {
            arg.push_display(&mut display, source);
            display.push_str(", ");
        }
        display.push(']');
        display
    }
}
