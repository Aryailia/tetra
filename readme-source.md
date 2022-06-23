# Tetra
{$ NAME = "Tetra"; PROJECT_NAME = "tetra-cli"; $}

{#
This is the source before it is compiled.
Please look at [README](./README.md). #}

{#
Rustdoc
<!--run: cargo run parse % README.md; rustdoc README.md --output "$TMPDIR"; #falkon "$TEMPDIR/README.html"
--> #}

This is a templating markup language designed to compile to other markup languages.
Although it can still use them as the backend, this language is designed to replace tools such as:
[RMarkdown](https://github.com/rstudio/rmarkdown),
[Obsidian](https://github.com/obsidianmd/obsidian-releases),
[Hugo](https://github.com/gohugoio/hugo)
and [Jupyter Notebook](https://github.com/jupyter/notebook).
At least that is the goal.

I encourage you to look at the source for this file [readme-source.md](./readme-source.md) to see it in action.
It is being used to augment Markdown.

At its core, this language is any lightweight markup augmented by the ability to run code and additional commands (like `cite`).
In other words, like Jupyter Notebook and RMarkdown, but with the freedom to choose your output language (i.e. not Markdown).
So a sample workflow is:

1. Write in the {$ NAME $} language with your favourite text editor
1. Use `{$ PROJECT_NAME $}` to compile to RMarkdown
1. Use R to compile to HTML
1. Preview with a browser like Falkon (because it can watch file changes)

Additionally, the library is setup that you can implement your own flavour of the language, i.e. make your own functions at compile-time.
Currently, there are no in-language ways to define functions (and this is likely a non-feature).
For reference, see [markup.rs](./src/run/markup.rs) for an example of how to implement your implementation.

# Why another markup language?

Markdown meets the demands of note takers, programmers, and bloggers through minimalism.
AsciiDoctor meets the needs of those who just want more, with with irreplaceable features like footnotes.
LaTeX is the industry standard for professional books and reports.
RMarkdown puts a human face on LaTeX, adding code execution for exploratory analysis, as well as any more publishable targets.

{$ NAME $} looks to occupy the same space as RMarkdown and be a tool that can fit into the workflow of more use cases.
Much of the space of computational notebooks (Org-mode Emacs, RMarkdown, Jupyter Notebook, Mathematica) are tied to a specific markup language (document writing) or a specific first-class coding language.
{$ NAME $} aims to be more flexible as it can target any language.

LaTeX is still the standard for document writing despite being as old as the internet itself and has acquired several quirks over the years.
These quirks are why so many solutions like RMarkdown have been developed.
Instead of replacing LaTeX as RMarkdown does, {$ NAME $} just marks up LaTeX with code execution making the experience more pleasant.
And still, when a better LaTeX solution is developed in the future, one can still use {$ NAME $}.

{# TODO An example of augmenting LaTeX likely has more explanatory power #}





# How to Use

## Installation

TODO (install rust, git clone, cargo install)

## Quickstart for default flavour

* Syntax is similar to Perl and shellscript and with ruby/rust-like function returns
  * Parenthesis `(` and `)` for function calls are optional.
  * Semicolons `;` end function calls.
  * Newlines does not act like semicolons.
  * If there is no semicolon (for the final function in a code cell) that means return it to be included in output
  * Piping `|` takes the output of the previous command and feeds it as the last argument to the next function.

`{{$ "a" | cite $}}` is equivalent to `{{$ cite "a" $}}`

* `{{$ $}}` are inline code embeds.
* `{{| |}}` are block code cells.
The text all the way up until the next `{{| |}}` is passed into the first command as the last argument and can be accessed via the period (`.`) symbol.
In other words, block code cells pipe STDIN to the first function.
Thus the following are equivalent.

`{{| run "sh"; cite "a" |}}` and `{{| ; run "sh", .; cite("a") |}}`


* `{$ "{# #}" $}` are block comments
* `.` represents STDIN, the text that immediately follows the block code cell. This cannot be used within block code cells.
* Like regular rust, both `{{$ $}}` and `{{| |}}` return the final value as its output unless you add a semicolon
  * Thus, the following is an alternative way to comment out a text cell:

```
Some text
{{| concat; |}}
Putting the a semicolon as the last means this entire text block.

{{| concat |}}
This will still display since there is no semicolon, thus runs `concat .`
```

{# @TODO: List of commands via rustdoc? #}

Flow control strucutres and namespace scoping for variables are not supported.
This is intended design, and you should use external programming languages for that.
However, it is likely possible to add this to your own flavour of the markup language.

{#

# Overview of the domain

## Markup Languages

There are 

* Lightweight Markup Languages
* Heavyweight Markup Languages
*

#}





# Applications

These help illustrate why this language is important and provide direction for code contributors for new features.

## Personal Knowledge Management (PKM)

{# footnote to Sonkhe Arhens book #}
The three key elements of a personal knowledge management system.

* Bidirectional links (links and knowing what links here)
* Frictionless writing and creation of entries
* Ways to tag and search for information given that knowledge is by nature are ever growing and impossible to pre-organise.

Many of these features are outside the purview of the markup itself, having more to do with the tooling around it.
Thus, {$ NAME $} does not provide much over tools like Obsidian, Roam Research, etc. other than being a more featured than Markdown.


Personal knowledge by nature becomes increasingly personalised as a user figures out their needs and acquires knowledge, ideally, calling for customised solutions.
Thus, {$ NAME $} is an essential tool for making a dent in that solution space.
To see how that space refer to [TODO link to the project I'm working on]().

## Static-Site Generation

The charm of static-site generators is there is virtually no JavaScript that weighs the site down, and that they allow you to write in a lightweight markup language.
What {$ NAME $} provides is code execution for more automation options.
In this sense, it does not provide much more over RMarkdown, except that it is less bloated, thus leading to fewer unexpected bugs.
For example, in RMarkdown, you cannot mix LaTeX citations with RMarkdown citations easily, as RMarkdown uses pandoc for its citations and runs the LaTeX parsing separately.

If all else fails, you can just compile to RMarkdown to make use of its tooling ecosystem, or to Markdown and make use of Hugo, Jekyll, etc.

## Reports and Academic Writing

The main reasons that people reach for LaTeX are over:

* Automatic formatting of citations and footnotes.
Access to biber/biblatex/etc. bibliography management tools.

* Access to math equations. For the Web, that means MathJax.

* Ability to reference sections and have the labeling of sections done automatically.

* For publishing purposes, e.g. the printing company accepts documents in LaTeX.

{$ NAME $}'s main selling point is, much like RMarkdown, it provides a much nicer interface for writers.
Unlike RMarkdown, {$ NAME $} is not tied to Markdown, and it is more transparent about toolchain dependencies.



The main reasons that people reach for computational notebooks or interactive notebook tools such as RMarkdown and Jupyter Notebook are:

* The ability to view live code execution as part of the exploration process of analysis
* Reproducible results
  * Maintain graphs and mentions of calculations from a single point
  * Documentation of code + build environment used to compile the code for graphs and calculations.

RMarkdown lets variables to be accessed across cell for R code.
(TODO) Currently, I plan on having {$ NAME $} allows variables to passed via capturing STDOUT, however this solution is likely not ideal.


## Templating languages

This is a space occupied by [Handlebars](https://github.com/sunng87/handlebars-rust), [Tera](https://github.com/Keats/tera), [Liquid](https://github.com/cobalt-org/liquid-rust), etc.
This also harkens to a time when Perl used CGI.pm to generate many of the websites of world wide web (unresearched claim).
Templating languages differ from regular markup languages in that they take data in addition to the source document.

I would be interested to see how far this use case could be taken without loops and just having code execution.



# Design

{# TODO: add citation #}

Jonathan Blow warns quite heavily against using formalised parsing languages (PEGs such as YACC, [Pest](https://github.com/pest-parser/pest), ANTLR, etc.), because while they lower the barrier to minimum viable product, they become a hindrance over the course of iterating on the language design.
This is a 

## Data Model Overview

This language parses documents as lists of cells-text cell pairs, code header and text body.
For example, imagine the following is an entire document.

```
body 1

{{| code header 2 |}}
Body 2


{{| code header 3 |}}
Body 3
```

This document has three cell pairs.
The first header is invisible (so that empty documents also have a header).


(When Async is supported) Each cell pair will be queued as a separate job.
The executor runs everything from top to bottom, running functions whose arguments are ready, looping until each function reports that it is done processing.
Then it knits (concatenates) everything together.

# Other projects

Personal projects

* Personal Knowledge Management CLI (todo link)
* Static-Site Generator (todo link). My [blog](aryailia.site) will be using this.

Other:

* [Keg](https://github.com/rwxrob/keg) is another personal knowledge management system, or zettelkasten-inspired system. I'm not sure where to learn about this project other than through YouTube.

* [Curated list](https://github.com/doanhthong/awesome-pkm) of personal knowledge management tools.

* [SILE](https://github.com/sile-typesetter/sile) is an very alpha, up-and-coming, performant replacement for LaTeX.
It only outputs to PDF currently and cannot do mathematics.

* [Tectonic](https://github.com/tectonic-typesetting/tectonic) is a LaTeX build system, based on XeTeX, but in Rust!


# TODO List

{# Targeting GitHub markdown which has checkboxes #}
* Syntax changes
  * [ ] Add support for table of contents
  * [ ] Add support for references
  * [ ] Add support for MathJax to SVG
  * [ ] Add optional parameters, e.g. `{{| cite("burton2004", style: "Harvard") |}}`
  * [ ] Change `LexType::EscapedChar( )` to `LexType::Literal( )` ?
  * [ ] Add a build cell for how to compile documents, e.g.
```
{{| compile_me_to "readme.md" | pandoc --output=readme.pdf; zathura readme.pdf  |}}
```

```
{{|
  compile_me_to "readme.rmd";
  system "Rscript -e \"rmarkdown::render('readme.rmd', output_file='readme.pdf')\""
|}}
```

  * [ ] Decide on method for passing data between programming languages, and between cells of the same programming language.

* Other
  * [ ] Have {$ PROJECT_NAME $} display a list of functions available for the default flavour?
  * [ ] Guarantee path is where the document lives?
  * [ ] GitHub Actions to compile this readme
  * [ ] Add access to Metadata (output file type, etc.) in user-defined functions
  * [ ] Improve error reporting on for building README.md
  * [ ] Add UTF-8 parsing tests
  * [ ] Is it possible to use PEGs to fuzz this language?



* Application Changes
  * [ ] Windows support (though it probably works as is). Test running PowerShell and cmd.exe external code
  * [ ] Make the executor async so documents can run scripts in parallel
  * [ ] Vim language integration (Display live code output, syntax highlighting etc.)
  * [ ] VS Code or [VSSodium](https://github.com/VSCodium/vscodium) language support
  * [ ] Web UI, perhaps via WebAssemby, to fully replace Jupyter Notebook


