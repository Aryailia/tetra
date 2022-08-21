#!/usr/bin/perl

use v5.28;                     # in perluniintro, latest bug fix for unicode
use feature 'unicode_strings'; # enable perl functions to use Unicode
use utf8;                      # source code in utf8
use strict 'subs';             # only allows declared functions
use warnings;

use JSON;
use File::Basename;

binmode STDIN, ':encoding(utf8)';
binmode STDOUT, ':encoding(utf8)';
binmode STDERR, ':encoding(utf8)';

#my $json = JSON->new->utf8->decode(<STDIN>);

my ($relpath, $input_path) = @ARGV;

basename($input_path) =~ /\.(.+?)$/;
my $ext = lc($1);

#run: ../../make.pl --local --force compile

sub main {
print(<<EOF);
<!DOCTYPE html>
<htmllang="en">
<head>
  <meta charset="UTF-8>
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="X-UA-Compatible" content="ie=edge">
  <link rel="stylesheet" href="$ENV{"DOMAIN"}/style.css">
  <title>Nondescript Title</title>
  <!--<script type="text/javascript"></script>
  <script type="text/javascript" src="src/app.js"></script>
  -->
</head>

<body><div class="structure-blog">
  <header class="sticky" id="top">
$ENV{"NAVBAR"}
  </header>
  <aside class="left">
  </aside>
  <main class="tabs">
@{[tab_bar_two_div(1, "checked")]}
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
        <pre>
@{[`cat \Q$input_path\E`]}
        </pre>
      </div>
    </div>
  </main>
  <aside class="right">
    <div>
      <b>Table of Contents</b>
      <div class="start-hide">
        left aside bar
      </div>
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
