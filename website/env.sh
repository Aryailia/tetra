#!/bin/sh

# Things you probably do not want to customise
export TEMPLATES="templates"                  #
export CACHE_DIR=".cache"                     #
export DOMAIN=""  # The base for links, different for file:// and web hosting
export LANGIFY_EXEC="../target/debug/langify" # Splits by lang
export TETRACLI="../target/debug/tetra-cli"   #

# Things you might want to customise
export   SOURCE_DIR="files"     # All other website files, fed to compile()
export   PUBLIC_DIR="public"    # Output. Where the webhost should point
export  POST_RELDIR="blog"      # The output for blog posts within $PUBLIC_DIR
export DEFAULT_LANG="en"

# '../blog' specific varaibles
export  PUBLISH_DIR="published" #
export LANGIFY_DATA="${CACHE_DIR}/langify.json"
export   PARSED_DIR="${CACHE_DIR}/parsed"  # Storage for intermediate step for 'post.pl' running from 'blog.pl'
export PARSED_PARTIAL="${CACHE_DIR}/metadata.partialjson"
