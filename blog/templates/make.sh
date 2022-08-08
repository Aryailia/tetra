#!/bin/sh

# This is the first version of make.pl, kept for posterity

NAME="$( basename "${0}"; printf a )"; NAME="${NAME%?a}"
WD="$( dirname "$0"; printf a )"; WD="${WD%?a}"
cd "${WD}" || { printf "Could not cd to directory of '%s'" "$0" >&2; exit 1; }
WD="$( pwd -P; printf a )"; WD="${WD%?a}"

exit_help() {
  printf %s\\n "SYNOPSIS" >&2
  printf %s\\n "  ${NAME} <JOB1> [JOB2..] " >&2

  printf %s\\n "" "DESCRIPTION" >&2
  printf %s\\n "  Works like a make file." >&2

  printf %s\\n "" "JOBS" >&2
  <"${NAME}" awk '
    /^    case "\$\{cmd\}"  # JOBS/ { run = 1; }
    /^    esac/                     { exit;    }
    run && /^      in|^      ;;/ {
      sub(/^ *in /, "  ", $0);
      sub(/^ *;; /, "  ", $0);
      sub(/\).*#/, "\t", $0);
      sub(/\).*/, "", $0);
      print $0;
    }
  ' | column -t -s "$( printf \\t )">&2
  exit 1
}


# Things you probably do not want to customise
FILES_TO_PROCESS_LIMIT=10000  # In case we go over the limit
TEMPLATES="templates"
CACHE_DIR=".cache"
DOMAIN=""  # For updating file paths

# Things you might want to customise
BLOG___DIR="${CACHE_DIR}/published" # Source for all blog posts, fed to postify()
OTHER__DIR="static"                 # All other website files, fed to compile()
PUBLIC_DIR="public"                 # Output. Where the webhost should point
POST_RELDIR="${PUBLIC_DIR}/blog"    # The output for blog posts
DEFAULT_LANG="en"





#run: ./% --local compile
my_make() {
  for cmd in "${@}"; do
    case "${cmd}"  # JOBS
      in clean)    # Removes $PUBLIC_DIR
        errln "" "Cleaning..."
        [ -d "${PUBLIC_DIR}" ] && rm -r "${PUBLIC_DIR}"

      ;; --local)  # Sets paths to be viewable in browser 'file:///home/...'
        errln "" "Set domain for local viewing"
        DOMAIN="${WD}/${PUBLIC_DIR}"

      ;; build)    # Build langify and tetra-cli
        errln "" "Building rust projects in debug..."
        cargo build --manifest-path "../Cargo.toml" 

      ;; website)  # Production or a web server "http://localhost:8000/..."
        errln "" "Building the website"
        mkdir -p "${PUBLIC_DIR}"
        walkdir_for_each_do "${OTHER__DIR}" "${OTHER__DIR}/" compile
        errln "${c_good} file(s) succeeded." \
              "${c_skip} file(s) skipped." \
              "${c_baad} file(s) failed."

      ;; blog)     # 
        errln "" "Building the posts"
        # Langify does mkdir
        # tetra-cli does not do 'mkdir' at the moment
        mkdir -p "${POST_RELDIR}"
        walkdir_for_each_do "${BLOG___DIR}" "${BLOG___DIR}/" compile_post

      ;; compile)    # Build everything but the rust
        my_make clean website blog

      ;; all)        # Sets domain to "/" for hosts
        my_make build compile
      ;; *)         exit_help
    esac
  done
  [ "$#" = 0 ] && exit_help

}

################################################################################
compile_post() {
  # $1: relative path of file

  langs="$( <"${BLOG___DIR}/${1}" run_langify "${CACHE_DIR}/langify" "${1}" )" \
    || exit "$?"

  lang_list="$( for lang in ${langs}; do
    if [ "${lang}" = "ALL" ]
      then printf %s\\n "${DEFAULT_LANG},${POST_RELDIR}/${DEFAULT_LANG}/${1}"
      else printf %s\\n "${lang},${POST_RELDIR}/${lang}/${1}"
    fi
  done )"

  for lang in ${langs}; do
    # tetra-cli does not do 'mkdir' at the moment
    output="${CACHE_DIR}/parsed/${lang}/${1}"

    if [ "${lang}" = "ALL" ]
      then out_lang="${DEFAULT_LANG}"
      else out_lang="${lang}"
    fi

    rel_out_path="${POST_RELDIR}/${lang}/${1}"
    mkdir -p "${CACHE_DIR}/parsed/${lang}" "${POST_RELDIR}/${lang}"

    navbar="$( "${TEMPLATES}/website/navbar.sh" "${DOMAIN}" "${rel_out_path}" "${lang}" )"

    run_tetra parse-and-json \
      "${CACHE_DIR}/langify/${lang}/${1}" \
      "${output}" \
    | navbar="${navbar}" \
      "${TEMPLATES}/website/post.pl" \
        "${output}" "${DOMAIN}" "${out_lang}" "${lang_list}" \
    >"${rel_out_path}"
  done

}


compile() {
  # $1: relative path of file

  from="${OTHER__DIR}/${1}"
  into="${PUBLIC_DIR}/${1}"
  into_stem="${into%.*}"
  ext="${1##*.}"

  mkdir -p "${into%/*}"
  if [ -h "${from}" ]; then
    abs_from="$( realpath "${from}"; printf a )"; abs_from="${abs_from%?a}"
    ln -s "${abs_from}" "${into}" && good || baad
  else
    case "${ext}"
      in html)  compile_into_html "run_tetra" "${1}" "${from}" "${into}" parse && good || baad
      ;; sh)    compile_into_html "sh" "${1}" "${from}" "${into}" && good || baad
      ;; scss)  sassc "${from}" "${into_stem}.css" && good || baad
      ;; *)     errln "Not processing: ${OTHER__DIR}/${1}"; skip
    esac
  fi
}


compile_into_html() {
  # $1: program used to compile $3 into $4 (change file extension to 'html')
  # $2: <path>
  # $3: ${OTHER__DIR}/<path>
  # $4: ${PUBLIC_DIR}/<path>
  # $@: other args, it goes: $1 "$@" $3
  program="${1}"
  rel_path="${2}"
  inp_path="${3}"
  out_path="${4}"
  shift 4

  NAVBAR="$( "${TEMPLATES}/website/navbar.sh" "${DOMAIN}" "${rel_path}" "@TODO lang" )" \
    DOMAIN="${DOMAIN}" \
    FOOTER="sitemap" \
    "${program}" "$@" "${inp_path}" \
  >"${out_path%.*}.html"
}

################################################################################
# Helpers
run_langify() {
  # $1: filetype
  # $2: output directory
  # $3: filename

  langify="../target/debug/langify"
  [ -x "${langify}" ] \
    || die FATAL 1 "No langify. Try first running:"  "    ${NAME} build"
  <&0 "${langify}" "$@" 
}

run_tetra() {
  tetra="../target/debug/tetra-cli"
  [ -x "${tetra}" ] \
    || die FATAL 1 "No tetra-cli. Try first running:"  "    ${NAME} build"
  "${tetra}" "$@"
}

# Count for logging
c_good=0
c_skip=0
c_baad=0
good() { c_good="$(( c_good + 1 ))"; }
skip() { c_skip="$(( c_skip + 1 ))"; }
baad() { c_baad="$(( c_baad + 1 ))"; }

# Follows symlinks
walkdir_for_each_do() {
  # $1: directory to recurse through
  # $2: prefix to remove from pathnames (typically "${1}/")
  # $3...: command to run, will add argument for file

  #[ "${1}" != "${1#/}" ] || die DEV 1 "'${1}' must be an absolute path"
  [ "${1}" != "${1#././}" ] && die DEV 1 "'${1}' must be in canonical form"
  [ -d "${1}" ] || die FATAL 1 "'${1}' is not a directory"

  # 'fe' for 'for each'
  fe_to_process="././${1}"
  fe_prefix_to_remove="${2}"
  shift 2
  fe_count=0
  while [ -n "${fe_to_process}" ]; do
    fe_dir="${fe_to_process%%././*}"
    fe_to_process="${fe_to_process#"${fe_dir}"}"
    fe_to_process="${fe_to_process#././}"
    fe_dir="${fe_dir#././}"

    if [ -n "${fe_dir}" ]; then
      for fe_node in "${fe_dir}"/* "${fe_dir}"/.[!.]* "${fe_dir}"..?*; do
        [ ! -e "${fe_node}" ] && continue
        fe_count="$(( fe_count + 1 ))"
        [ "${fe_count}" -gt "${FILES_TO_PROCESS_LIMIT}" ] && die FATAL 1 \
          "Files processed in '${1}' > '${FILES_TO_PROCESS_LIMIT}'" \
          "Increase \${FILES_TO_PROCESS_LIMIT} inside of '${NAME}'"

        if [ ! -h "${fe_node}" ] && [ -d "${fe_node}" ]; then
          fe_to_process="${fe_to_process}././${fe_node}"
          continue
        fi
        "$@" "${fe_node#"${fe_prefix_to_remove}"}" || exit "$?"
      done
    fi
  done
}

outln() { printf %s\\n "$@"; }
errln() { printf %s\\n "$@" >&2; }
die() { printf %s "${1}: " >&2; shift 1; printf %s\\n "$@" >&2; exit "${1}"; }
eval_escape() { <&0 sed "s/'/'\\\\''/g;1s/^/'/;\$s/\$/'/"; }
################################################################################

my_make "$@"
