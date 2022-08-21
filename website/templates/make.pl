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

# Things you probably do not want to customise
my $FORCE = 0;                               # @TODO

#run: ../make.sh --local all
my %cmds = (
  "--local" => ["Set \$DOMAIN to for local links", sub {
    say STDERR "Set domain for local viewing" if not exists $ENV{'LOCAL'};
    $ENV{'DOMAIN'} = cwd() . "/$ENV{'PUBLIC_DIR'}";
  }],
  "--force" => ["Ignores checking if source file is newer than target file", sub {
    say STDERR "Always compile without checking last modified date" if not exists $ENV{'FORCE'};
    $FORCE = 1;
  }],

  "clean" => ["Removes \$PUBLIC_DIR", sub {
    say STDERR "Removing cache and public dir...";
    `rm -r \Q$ENV{'CACHE_DIR'}\E` if -d $ENV{'CACHE_DIR'};
    `rm -r \Q$ENV{'PUBLIC_DIR'}\E` if -d $ENV{'PUBLIC_DIR'};
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

# Special case for '../blog'
# Since 'blog.pl' executes commands one by one, the environment does not carry
# over, thus we have to set this explicitly given an environment variable
# set in 'blog.sh'
$cmds{'--force'}[1]() if exists $ENV{'FORCE'} and $ENV{'FORCE'};
$cmds{'--local'}[1]() if exists $ENV{'LOCAL'} and $ENV{'LOCAL'};

################################################################################
sub build_website {
  say STDERR "Compiling the website...";
  my @files = enumerate_files_as_relpaths($ENV{'SOURCE_DIR'});

  my ($good, $bad, $skip) = (0, 0, 0);

  foreach my $relpath (@files) {
    my $inp_path = "$ENV{'SOURCE_DIR'}/$relpath";
    my $out_path = "$ENV{'PUBLIC_DIR'}/$relpath";
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
        compile_into_html($ENV{'TETRACLI'}, $relpath, $inp_path, "$out_relstem.html", "parse");
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
      compile_into_html("$ENV{'TEMPLATES'}/website/post.pl",  # program
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
  $ENV{'NAVBAR'} = navbar($relpath, "en");
  $ENV{'FOOTER'} = "sitemap";
  #also pass $ENV{'DOMAIN'};
  system("/bin/sh", "-c", 'out="$1"; shift 1; "$@" >"${out}"',
    "_", $out_path,
    $program, @_[4..$#_], $inp_path,
  );
}


################################################################################
sub navbar {
  my ($relpath, $lang) = @_;
  return `\Q$ENV{'TEMPLATES'}/website/navbar.sh\E \Q$relpath\E \Q$lang\E`;
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
