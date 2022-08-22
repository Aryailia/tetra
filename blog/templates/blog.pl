#!/usr/bin/perl

use v5.28;                     # in perluniintro, latest bug fix for unicode
use feature 'unicode_strings'; # enable perl functions to use Unicode
use utf8;                      # source code in utf8
use strict 'subs';             # only allows declared functions
use warnings;

use File::Basename ('dirname', 'basename');
use File::Find 'find';
use File::Path 'make_path';
use File::Copy 'cp';
use JSON;
use List::Util 'max';

binmode STDIN, ':encoding(utf8)';
binmode STDOUT, ':encoding(utf8)';
binmode STDERR, ':encoding(utf8)';

# $MAKE reads from "files/" and "templates/"
my $MAKE         = "./make.sh";
my $STATIC_DIR   = "static";
my $LANG_INP_DIR = $ENV{'PUBLISH_DIR'};
my $LANG_OUT_DIR = "$ENV{'SOURCE_DIR'}/$ENV{'POST_RELDIR'}";

$ENV{'DOMAIN'} = 0;
$ENV{'FORCE'}  = 0;

#run: ../blog.sh --local all

# We are wrapping the commands that build website with `split_langs()`
# This is so `$MAKE build` can occur by itself
my %cmds;
%cmds = (
  "--local" => ["Refer to $MAKE help", sub {
    say STDERR "Set domain for local viewing";
    $ENV{'LOCAL'} = 1;  # Signal to 'make.pl'
  }],
  "--force" => ["Refer to $MAKE help", sub {
    say STDERR "Always compile without checking last modified date";
    $ENV{'FORCE'} = 1;  # Signal to 'make.pl'
  }],
  "clean" => ["Remove '$ENV{'SOURCE_DIR'}'", sub {
    say STDERR "$cmds{'clean'}[0]...";
    `rm -r \Q$ENV{'SOURCE_DIR'}\E`;
    `\Q$MAKE\E clean`;
  }],
  "copy" => ["Copy files from '$STATIC_DIR' to '$ENV{'SOURCE_DIR'}'", sub {
    say STDERR "$cmds{'copy'}[0]...";
    copy_static();
  }],
  "langify" => ["Applies langify on files in $LANG_INP_DIR", sub {
    say STDERR "Splitting the posts in '$LANG_INP_DIR' to '$LANG_OUT_DIR'";
    split_langs();
  }],
  "website" => ["Refer to $MAKE help", sub {
    my_make('langify');
    `\Q$MAKE\E website`;
  }],
  "all" => ["Clean and build everything", sub {
    my_make("clean");
    `\Q$MAKE\E build`;
    my_make("copy", "website");
  }],
);


sub copy_static {
  foreach my $p (walk($STATIC_DIR)) {
    my ($a, $b) = ("$STATIC_DIR/$p", "$ENV{'SOURCE_DIR'}/$p");
    make_path(dirname($b));
    cp($a, $b) or die "$?: Cannot copy '$a' -> '$b'";
  }
}

sub split_langs() {
  make_path($ENV{'CACHE_DIR'});

  my %metadata;
  foreach my $p (walk($LANG_INP_DIR)) {
    my $lang_str = `\Q$ENV{'LANGIFY_EXEC'}\E \\
      <\Q$LANG_INP_DIR/$p\E \\
      --default-lang \Q$ENV{'DEFAULT_LANG'}\E \\
      \Q$LANG_OUT_DIR\E \Q$p\E`;
    my @lang_list = split /\s/, $lang_str;
    $metadata{$p} = \@lang_list;
  }

  my $json = encode_json \%metadata;
  open FH, '>', "$ENV{'LANGIFY_DATA'}";
  print FH $json;
  close FH;
}


sub walk {
  my $dir = $_[0];
  my @files;

  find({
    wanted => sub {
      my $path = $File::Find::name;
      return if $path eq $dir;
      # Need basename because `find()` changes directory
      my $base = basename($path);
      return if (!-l $base) && (-d $base);
      push @files, substr($path, length($dir) + 1);
    },

    # Not sure exactly what this needs to be, but enables `perl -T %`
    untaint => sub {},
  }, $dir);
  return @files;
}

################################################################################
sub my_make {
  if ($#_  == -1) {
    help();
  } else {
    foreach (@_) {
      if (exists($cmds{$_})) {
        $cmds{$_}[1]();
      } else {
        `\Q$MAKE\E \Q$_\E`;
      }
    }
  }
}

sub help {
  print(<<EOF);
SYNOPSIS
  $0 <subcommand1> [<subcommand2> [..]]

DESCRIPTION
  Functions much like a Makefile

SUBCOMMANDS
EOF
  my $len = max(map { length $_ } keys %cmds);
  for my $key (keys %cmds) {
    printf "  %-${len}s    %s\n", $key, $cmds{$key}[0];
  }
  exit 1;
}

my_make(@ARGV);
