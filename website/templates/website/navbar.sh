#!/usr/bin/env sh

# $DOMAIN (local vs deployment)
# $1: Relative path from public for the file for which this header is for
# $2: Language (can be blank, probably only using for blog/index.html)


entry() {
  # $1: relpath of current file
  # $2: relpath to test against
  # $3: text to display in navbar
  # $4: url to link to
  if [ "${1}" = "${2}" ]
    then printf '<span class="current">'
    else printf '<span>'
  fi
  printf        '<a href="%s">%s</a></span>' "${DOMAIN}/${4}" "${3}"
}
s='    '
tag="${2:+"#${2}"}"  # Add the anchor if not blank

outln() { printf %s\\n "$@"; }


outln "${s}"'<nav id="top" class="link-hover-only-underline">'
outln "${s}  $(    entry "${1}" "index.html"    "Home"     "" )<!--"
outln "${s}  -->$( entry "${1}" "projects.html" "Projects" "projects.html" )<!--"
outln "${s}  -->$( entry "${1}" "notes.html"    "Notes"    "notes.html" )<!--"
outln "${s}  -->$( entry "${1}" "blog.html"     "Blog"     "blog${tag}" )<!--"
outln "${s}  -->$( entry "${1}" "about.html"    "About"    "about.html" )<!--"
outln "${s}--></nav>"
