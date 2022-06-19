//run: cargo test -- --nocapture
/******************************************************************************
 * Errors
 ******************************************************************************/

macro_rules! my_error {
    ($( $error_name:ident => $message:literal, )*) => {

        #[derive(Debug)]
        #[repr(usize)]
        pub enum MyError {
            $( $error_name, )*
        }

        const ERROR_MESSAGES: [&str; 0 $( + my_error!(@to_one $message) )*] = [
            $( $message, )*
        ];

    };
    (@to_one $_:literal) => { 1 };
}

my_error! {
    IncompleteComment => "You did not close the Comment block with '#}' before the end-of-file",
    IncompleteUseSource => "You did not close the UseSource block with '%}' before the end-of-file",
    IncompleteExpression => "You did not close the Expression block with '}}' before the end-of-file",
    MismatchingParens => "Mismatched bracket or parentheses [] ()",
    NoOpeningParen => "There is no associated opening bracket or parentheses [] ()",
    NoBlankArgs => "",
    NonAlphabeticIdentStart => "Identifiers must start with A-Z or a-z",

    NoTuples => "This language does not support tuples, use square brackets [] \
        to display lists instead",
    FunctionsHaveNoParens => "Functions do not have parentheses () in this \
        language; parentheses always evalute to values after parsing. Put a \
        space between the variable identifier and the parenthensis.",

    // ParserErrors
    BlocksMustStartWithAFunction => "Blocks must start with a \
        function because you are feeding the next text \
        as the first argument",

    // These two probably should error at the 'assign' function
    VariableAfterStdin => "{% %} blocks use input afterwards as the first \
        argument (as if they were piped. Thus this must be interpreted as a
        function despite you defining it as a variable.",
    VariableAfterPipe => "The identifier after a pipe '|' must be a \
        function. Your variable conflicts with this name.",
    StartStatementWithPipe => "You tried to start a statement with a pipe. \
        You must start with a literal, variable, or function. If you wanted \
        to pipe the next block use the dot instead,\ne.g. `. | <function>`",
    Temp => "todo",
}

