#!/bin/bash

set -e

if [ "$1" != "" ]; then
	~/npm/node_modules/prettier/bin-prettier.js --write --prose-wrap=always $1;
	exit
fi

# Ignore notes; they're not important
for x in `find | grep '.md$' | grep -v design/notes | grep -v TODO_ | grep -v CHANGELOG.md`; do
	~/npm/node_modules/prettier/bin-prettier.js --write --prose-wrap=always $x;
done
