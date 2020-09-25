#!/bin/bash

set -e
~/npm/node_modules/prettier/bin-prettier.js --write --prose-wrap=always $1

# Format everything:
# for x in `find book/src/ | grep '\.md'`; do ./format_md.sh $x; done; git checkout book/src/project/CHANGELOG.md
