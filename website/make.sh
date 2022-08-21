#!/bin/sh

WD="$( dirname "$0"; printf a )"; WD="${WD%?a}"
cd "${WD}" || { printf "Could not cd to directory of '%s'" "$0" >&2; exit 1; }
WD="$( pwd -P; printf a )"; WD="${WD%?a}"

. ./env.sh || exit "$?"
perl "${TEMPLATES}/make.pl" "$@"
#perl -I "${WD}" "${TEMPLATES}/make.pl" "$@"
