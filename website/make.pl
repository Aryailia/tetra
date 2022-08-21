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
my $FORCE = 0;                               # @TODO
my $LANGIFY = "../target/debug/langify";     # Splits by lang
my $TETRACLI = "../target/debug/tetra-cli";  #

# Things you might want to customise
my $BLOG___DIR = "$CACHE_DIR/published";  # Source for all blog posts, fed to postify()
my $OTHER__DIR = "files";                 # All other website files, fed to compile()
my $PUBLIC_DIR = "public";                # Output. Where the webhost should point
my $POST_RELDIR = "blog";                 # The output for blog posts within $PUBLIC_DIR
my $DEFAULT_LANG = "en";



#run: perl % --local all
my %cmds = (
  "--local" => ["Set \$DOMAIN to for local links", sub {
    say STDERR "", "Set domain for local viewing";
    $DOMAIN = "$cwd/$PUBLIC_DIR";
  }],
  "--force" => ["Ignores checking if source file is newer than target file", sub {
    say STDERR "", "Always compiling";
    $FORCE = 1;
  }],

  "clean" => ["Removes \$PUBLIC_DIR", sub {
    say STDERR "Removing public dir (WIP on cache too)...";
    `rm -r \Q$CACHE_DIR\E` if -d $CACHE_DIR;
    `rm -r \Q$PUBLIC_DIR\E` if -d $PUBLIC_DIR;
  }],

  "build" => ["Build langify and tetra-cli, the rust projects", sub {
    say STDERR "", "Building rust projects for debug build...";
    `cargo build --manifest-path "../Cargo.toml"`
  }],

  "website" => ["Clean and build everything", sub { build_website() }],

  "all" => ["Clean and build everything", sub {
    my_make("clean", "build", "website");
  }],
);

$cmds{'--force'}[1]() if exists $ENV{'FORCE'} and $ENV{'FORCE'};
$cmds{'--local'}[1]() if exists $ENV{'DOMAIN'} and $ENV{'DOMAIN'};

################################################################################
sub build_website {
  say STDERR "Compiling the website...";
  my @files = enumerate_files_as_relpaths($OTHER__DIR);

  my ($good, $bad, $skip) = (0, 0, 0);

  foreach my $relpath (@files) {
    my $inp_path = "$OTHER__DIR/$relpath";
    my $out_path = "$PUBLIC_DIR/$relpath";
    "/$relpath" =~ /^.*\/.+\.(.+?)$/;  # For some reason non-greedy not working
    my $ext = lc($1);
    my $out_relstem = substr($out_path, 0, -length($ext) - 1);
    make_path(dirname("$out_path"));

    if (-l $inp_path && -d $inp_path) {
      my $abs_path = abs_path($inp_path);
      unlink $out_path;  # delete file
      `ln -s \Q$abs_path\E \Q$out_path\E`;
      $? == 0 ? ($good += 1) : ($bad += 1);

    } elsif ($ext =~ /svg|png|gif|jpe?g/) {
      `cp \Q$inp_path\E \Q$out_path\E`;
      $? == 0 ? ($good += 1) : ($bad += 1);

    } elsif ($ext eq "html") {
      if (is_left_newer($inp_path, "$out_relstem.html")) {
        compile_into_html($TETRACLI, $relpath, $inp_path, "$out_relstem.html", "parse");
        $? == 0 ? ($good += 1) : ($bad += 1);
      } else {
        $skip += 1;
      }

    } elsif ($ext eq "sh") {
      if (is_left_newer($inp_path, "$out_relstem.html")) {
        compile_into_html("/bin/sh", $relpath, $inp_path, "$out_relstem.html");
        $? == 0 ? ($good += 1) : ($bad += 1);
      } else {
        $skip += 1;
      }

    } elsif ($ext eq "scss") {
      if (is_left_newer($inp_path, "$out_relstem.css")) {
        `sassc \Q$inp_path\E \Q$out_relstem.css\E`;
        $? == 0 ? ($good += 1) : ($bad += 1);
      } else {
        $skip += 1;
      }

    } elsif ($ext eq "adoc" || $ext eq "md") {
      compile_into_html("$TEMPLATES/website/post.pl",  # program
        $relpath, $inp_path, "$out_relstem.html",      # required args
        $relpath,                                      # extra args to pass to 'post.pl'
      );
      $? == 0 ? ($good += 1) : ($bad += 1);

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

sub compile_into_html {
  my ($program, $relpath, $inp_path, $out_path) = @_;
  my $navbar = navbar($relpath, "en");
  my $footer = "sitemap";

  system("/bin/sh", "-c",
    'out="$1"; shift 1
    export NAVBAR="$1"; export DOMAIN="$2"; export FOOTER="$3"; shift 3
    "$@" >"$out"',
    "_", $out_path,
    $navbar, $DOMAIN, $footer,
    $program, @_[4..$#_], $inp_path,
  );
  #`NAVBAR=\Q$navbar\E DOMAIN=\Q$DOMAIN\E FOOTER=\Qsitemap\E \\
  #  \Q$program\E \Q@params\E \Q$inp_path\E \\
  #>\Q$out_path\E`;
}


################################################################################
sub navbar {
  my ($relpath, $lang) = @_;
  return `\Q$TEMPLATES/website/navbar.sh\E \Q$DOMAIN\E \Q$relpath\E \Q$lang\E`;
}

sub is_left_newer {
  my ($l, $r) = @_;
  return 1 if $FORCE or not -e $r;
  return (stat $l)[9] > (stat $r)[9] ? 1 : 0;
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
  if ($#_  == -1) {
    help();
  } else {
    foreach (@_) {
      if (exists($cmds{$_})) {
        $cmds{$_}[1]();
      } else {
        say "Unknown subcommand '$_'";
        help();
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
