#!/usr/bin/perl

# Since this is run from 'make.pl', the working dir is the same as 'make.pl'

use v5.28;                     # in perluniintro, latest bug fix for unicode
use feature 'unicode_strings'; # enable perl functions to use Unicode
use utf8;                      # source code in utf8
use strict 'subs';             # only allows declared functions
use warnings;

use JSON;
use File::Basename;
use File::Path 'make_path';

binmode STDIN, ':encoding(utf8)';
binmode STDOUT, ':encoding(utf8)';
binmode STDERR, ':encoding(utf8)';

#my $json = JSON->new->utf8->decode(<STDIN>);

#run: ../../blog.sh --local all

sub parse {
  my ($relpath, $input_path) = @_;

  open LJ, "<", $ENV{'LANGIFY_DATA'};
  my $langify_data = decode_json <LJ>;
  close LJ;

  my ($post_reldir, $lang_list, $lang);
  if ($relpath =~ m|^blog/(?:(.*)/)?([^/]+)/([^/]+)$|) {
    $post_reldir = defined $1 ? "blog/$1" : "blog";
    my $key = defined $1 ? "$1/$3" : $3;
    $lang_list = $langify_data->{$key} or die "Could not find '$key' in '$ENV{'LANGIFY_DATA'}'";
    $lang = $2;
  } else {
    die "DEV: Blog post not in the expected file path format";
  }

  make_path(dirname("$ENV{'PARSED_DIR'}/$relpath"));
  my $parsed_path = "$ENV{'PARSED_DIR'}/$relpath";
  my $json_str = `\Q$ENV{'TETRACLI'}\E parse-and-json \Q$input_path\E \Q$parsed_path\E`;

  open PJ, ">>", $ENV{'PARSED_PARTIAL'};
  print PJ ",$json_str";
  close PJ;

  return ($input_path, $lang, $lang_list, $json_str);
}

sub main {
  my ($input_path, $lang, $lang_list, $json_str) = parse @ARGV;

  my $json = decode_json($json_str);
  my %attributes = %{$json->{"attributes"}};
  basename($input_path) =~ m|([^/])*\.([^\.]+)$|
    or die "'$input_path' does not a filestem and/or extension";
  my $stem = $1;
  my $ext = lc($2);

  # read from JSON
  my $title        = ($attributes{"title"} or "");
  my $author       = ($attributes{"author"} or "");
  my $date_created = ($attributes{"date-created"} or ""); $date_created =~ s/ \d\d?:.*$//;
  my $date_updated = ($attributes{"date-updated"} or ""); $date_updated =~ s/ \d\d?:.*$//;

print(<<EOF);
<!DOCTYPE html>
<html lang="$lang">
<head>
  <meta charset="UTF-8>
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="X-UA-Compatible" content="ie=edge">
  <link rel="stylesheet" href="$ENV{"DOMAIN"}/style.css">
  <title>$title</title>

  <!--<script type="text/javascript"></script>
  <script type="text/javascript" src="src/app.js"></script>
  -->
</head>

<body><div class="structure-blog">
  <header class="sticky" id="top">
$ENV{"NAVBAR"}
  </header>
  <aside class="left">
    <div>Created: ${date_created}</div>
EOF

################################################################################
# Left - Languages and Series
my $post_reldir = "$ENV{'DOMAIN'}/$ENV{'POST_RELDIR'}";
foreach my $l (@$lang_list) {
  say qq{    <a href="$post_reldir/$l/$stem.html">$l</a>} if $l ne $lang;
}


################################################################################
# Main
print(<<EOF);
  </aside>
  <main class="tabs">
@{[tab_bar_two_div(1, "checked")]}
    <h1>${title}</h1>
    <div>Last Updated: ${date_updated}</div>
EOF
if ($ext eq "adoc") {
  system("asciidoctor", $input_path, "--out-file", "-",
    "--no-header-footer",
    "--attribute", "source-highlighter=pygments",
    "--attribute", "webfonts!",
    "--attribute", "imagesdir=$ENV{'DOMAIN'}/images",
  );
} elsif (1) {
}

print(<<EOF);
      </div>
    </div>
@{[tab_bar_two_div(2, "")]}
        <pre><code>
@{[`cat \Q$input_path\E`]}
        </code></pre>
      </div>
    </div>
  </main>
EOF



################################################################################
# Right - Table of Contents
print(<<EOF);
  <aside class="right">
    <div>
      <b>Table of Contents</b>
      <div class="start-hide">
<ul>
EOF

my $curr = 1;

for  (@{$json->{"outline"}}) {
  my ($level, $heading) = @$_;
  #say STDERR $level, $heading;
  if ($level > $curr) {
    say "<ul>"
  } elsif ($level < $curr) {
    say "</ul>"
  }
  say "<li>$heading</li>";
  $curr = $level;
}
say "</ul>" if $curr > 1;

print(<<EOF);
</ul>
    </div>
  </aside>
  <footer>
$ENV{"FOOTER"}
  </footer>
</div></body>
</html>
EOF
}

################################################################################
# Helpers

# Adds two divs
sub tab_bar_two_div {
  my @tabs = ("Display", "Source");
  my ($select, $checked) = @_;

  my $output = "";
  my $spaces = "      ";
  my $i = 0;

  $output .=   qq{$spaces<input class="tab-head" id="tab$select" name="main-tab-bar" type="radio" $checked>};
  $output .=   qq{$spaces<div>};
  foreach my $text (@tabs) {
    $i += 1;
    my $add_class = ($i == $select) ? "chosen" : "      ";
    $output .= qq{$spaces  <label class="tab-label $add_class" for="tab$i">$text</label>};
  }
  $output .=   qq{$spaces  <div class="tab-body">};
  return $output;
}

main();
