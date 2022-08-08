#!/usr/bin/perl

use v5.28;                     # in perluniintro, latest bug fix for unicode
use feature 'unicode_strings'; # enable perl functions to use Unicode
use utf8;                      # source code in utf8
use strict 'subs';             # only allows declared functions
use warnings;

use Cwd 'abs_path', 'cwd';
use File::Basename 'dirname', 'basename';
use File::Find 'find';
use File::Path 'make_path';
use JSON;
use List::Util 'max';

binmode STDIN, ':encoding(utf8)';
binmode STDOUT, ':encoding(utf8)';
binmode STDERR, ':encoding(utf8)';

chdir dirname($0);
my $cwd = cwd();

# Things you probably do not want to customise
my $TEMPLATES = "templates";                 #
my $CACHE_DIR = ".cache";                    #
my $DOMAIN = "";  # The base for links, different for file:// and web hosting
my $LANGIFY = "../target/debug/langify";     # Splits by lang
my $TETRACLI = "../target/debug/tetra-cli";  #

# Things you might want to customise
my $BLOG___DIR = "$CACHE_DIR/published";  # Source for all blog posts, fed to postify()
my $OTHER__DIR = "static";                # All other website files, fed to compile()
my $PUBLIC_DIR = "public";                # Output. Where the webhost should point
my $POST_RELDIR = "blog";                 # The output for blog posts within $PUBLIC_DIR
my $DEFAULT_LANG = "en";



#run: perl % clean --local compile
my %cmds = (
  "--local" => ["description", sub {
    say STDERR "", "Set domain for local viewing";
    $DOMAIN = "$cwd/$PUBLIC_DIR";
  }],

  "clean" => ["Removes \$PUBLIC_DIR", sub {
    say STDERR "Removing public dir (WIP on cache too)...";
    `rm -r \Q$PUBLIC_DIR\E` if -d $PUBLIC_DIR;
  }],

  "build" => ["Build langify and tetra-cli, the rust projects", sub {
    say STDERR "", "Building rust projects for debug build...";
    `cargo build --manifest-path "../Cargo.toml"`
  }],

  "blog" => ["Build the blog posts", sub { build_blog() }],

  "website" => ["Clean and build everything", sub { build_website() }],
  "compile" => ["Build everything but rust", sub {
    my_make("website", "blog");
  }],

  "all" => ["Clean and build everything", sub {
    my_make("clean", "build", "compile");
  }],
);

sub build_website {
  say STDERR "Compiling the website...";
  my @files = enumerate_files_as_relpaths($OTHER__DIR);

  my ($good, $bad, $skip) = (0, 0, 0);

  foreach my $relpath (@files) {
    my $inp_path = "$OTHER__DIR/$relpath";
    my $out_path = "$PUBLIC_DIR/$relpath";
    "/$relpath" =~ /^.*\/.+\.(.+?)$/;  # For some reason non-greedy not working
    my $ext = $1;
    my $out_stem = substr($out_path, 0, -length($ext) - 1);
    make_path dirname("$out_path");

    if (-l $inp_path) {
      my $abs_path = abs_path($inp_path);
      unlink $out_path;  # delete file
      `ln -s \Q$abs_path\E \Q$out_path\E`;
      $? == 0 ? ($good += 1) : ($bad += 1);

    } elsif ($ext eq "html") {
      if (is_left_newer($inp_path, "$out_stem.html")) {
        compile_into_html($TETRACLI, $relpath, $inp_path, "$out_stem.html", "parse");
        $? == 0 ? ($good += 1) : ($bad += 1);
      } else {
        $skip += 1;
      }

    } elsif ($ext eq "sh") {
      if (is_left_newer($inp_path, "$out_stem.html")) {
        compile_into_html("/bin/sh", $relpath, $inp_path, "$out_stem.html");
        $? == 0 ? ($good += 1) : ($bad += 1);
      } else {
        $skip += 1;
      }

    } elsif ($ext eq "scss") {
      if (is_left_newer($inp_path, "$out_stem.css")) {
        `sassc \Q$inp_path\E \Q$out_stem.css\E`;
        $? == 0 ? ($good += 1) : ($bad += 1);
      } else {
        $skip += 1;
      }

    } else {
      say STDERR "No handle for processing: $inp_path";
      $skip += 1;
    }
  }

  say STDERR "Processed ", $#files + 1, " file(s)";
  say STDERR "  Succeeded with $good file(s)";
  say STDERR "  Skipped $skip file(s)";
  say STDERR "  Failed $bad file(s)";
}

sub is_left_newer {
  my ($l, $r) = @_;
  return 1 if not -e $r;
  return (stat $l)[9] > (stat $r)[9] ? 1 : 0;
}

sub compile_into_html {
  my ($program, $relpath, $inp_path, $out_path) = @_;
  my $navbar = navbar($relpath, "en");
  my @params = $#_ >= 4 ? @_[4,] : ();
  `NAVBAR=\Q$navbar\E DOMAIN=\Q$DOMAIN\E FOOTER=\Q\E \\
    \Q$program\E \Q@params\E \Q$inp_path\E \\
  >\Q$out_path\E`;
}


sub build_blog {
  say STDERR "Compiling the posts...";
  my @files = enumerate_files_as_relpaths($BLOG___DIR);
  my ($good, $skip) = (0, 0);

  foreach my $relpath (@files) {
    my $inp_path = "$BLOG___DIR/$relpath";
    if (not is_left_newer($inp_path, "$PUBLIC_DIR/$POST_RELDIR/en/$relpath")) {
      $skip += 1;
      next
    }

    my $lang_str = `<\Q$BLOG___DIR/$relpath\E \\
      \Q$LANGIFY\E \Q$CACHE_DIR/langify\E \Q$relpath\E
    `;
    my @inp_langs = split /\s+/, $lang_str;
    my @out_langs = map { $_ eq "ALL" ? $DEFAULT_LANG : $_ } @inp_langs;
    @out_langs = map { [$_, "$POST_RELDIR/$_/$relpath"] } @out_langs;
    my %langs = map { ref eq 'ARRAY' ? @$_ : $_ } @out_langs;
    my $other_langs = encode_json \%langs;


    foreach (@inp_langs) {
      my $langify_path = "$CACHE_DIR/langify/$_/$relpath";
      my $lang = $_ eq "ALL" ? $DEFAULT_LANG : $_;
      my $parse_path = "$CACHE_DIR/parsed/$lang/$relpath";
      my $out_relpath = "$POST_RELDIR/$lang/$relpath";
      my $out_relstem = $out_relpath;
      $out_relstem =~ s/\.([^.]+)$//;

      make_path(dirname($parse_path));
      make_path(dirname("$PUBLIC_DIR/$out_relpath"));

      my $navbar = navbar($out_relpath, $lang);
      my $json_str = `\Q$TETRACLI\E parse-and-json \Q$langify_path\E \Q$parse_path\E`;

      `navbar=\Q$navbar\E \Q$TEMPLATES/website/post.pl\E \\
        \Q$parse_path\E \Q$DOMAIN\E \Q$lang\E \Q$other_langs\E \Q$json_str\E \\
      >\Q$PUBLIC_DIR/$out_relstem.html\E `;

      die "Error processing '$BLOG___DIR/$relpath' -> '$PUBLIC_DIR/$out_relpath'"
        if $? != 0;

      $good += 1;
    }
  }

  say STDERR "Processed ", $#files + 1, " file(s)";
  say STDERR "  Successfully created $good output file(s)";
  say STDERR "  Skipped              $skip source file(s)";
}


################################################################################
sub navbar {
  my ($relpath, $lang) = @_;
  return `\Q$TEMPLATES/website/navbar.sh\E \Q$DOMAIN\E \Q$relpath\E \Q$lang\E`;
}

sub enumerate_files_as_relpaths {
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


sub my_make {
  foreach (@_) {
    if (exists($cmds{$_})) {
      $cmds{$_}[1]();
    } else {
      say "Unknown subcommand '$_'";
      help();
    }
  }
}

sub help {
  print(<<EOF);
SYNOPSIS
  $0 

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
