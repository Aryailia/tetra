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


#my $stdin = <STDIN>;
#say $stdin;
#my $json = JSON->new->utf8->decode(<STDIN>);
#say $json->{"outline"}->[2][0];

my $templates_dir = "";
my ($input_path, $domain, $lang, $other_langs, $json_str) = @ARGV;
my $json = decode_json($json_str);

basename($input_path) =~ /\.(.+?)$/;
my $ext = lc($1);

# read from JSON
my $title = "WIP";
my $date_updated = "WIP";

#run: ../../make.pl --local compile


print(<<EOF);
<!DOCTYPE html>
<html lang="$lang">
<head>
  <meta charset="UTF-8>
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="X-UA-Compatible" content="ie=edge">
  <link rel="stylesheet" href="$domain/style.css">
  <title>$title</title>

  <!--<script type="text/javascript"></script>
  <script type="text/javascript" src="src/app.js"></script>
  -->
</head>

<body><div class="structure-blog">
  <header class="sticky" id="top">
$ENV{"navbar"}
  </header>
  <aside class="left">
EOF

my %lang_pathmap = %{decode_json($other_langs)};
foreach my $l (keys %lang_pathmap) {
  say qq{    <a href="$lang_pathmap{$l}">$l</a>} if $l ne $lang;
}


print(<<EOF);
  </aside>
  <aside class="right">
  b

  </aside>
  <main>
    <h1>${title}</h1>
    <div>Last Updated: ${date_updated}</div>
EOF

if ($ext eq "adoc") {
  system("asciidoctor", $input_path, "--out-file", "-",
    "--no-header-footer",
    "--attribute", "source-highlighter=pygments",
    "--attribute", "webfonts!",
    "--attribute", "imagesdir=$domain/images",
  );
} elsif (1) {
}

print(<<EOF);
  </main>
  <footer>
<!-- INSERT: footer -->
  </footer>
</div></body>
</html>
EOF



