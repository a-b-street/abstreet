#!/bin/bash
# Manually run after pushing the Github release

set -e

OLD_MAJOR=0
OLD_MINOR=3
OLD_PATCH=$1

NEW_MAJOR=0
NEW_MINOR=3
NEW_PATCH=$2

if [ "$OLD_PATCH" == "" ] || [ "$NEW_PATCH" == "" ]; then
	echo Missing args;
	exit 1;
fi

# This assumes https://github.com/a-b-street/docs is checked out at ~/docs
perl -pi -e "s/${OLD_MAJOR}_${OLD_MINOR}_${OLD_PATCH}/${NEW_MAJOR}_${NEW_MINOR}_${NEW_PATCH}/g" README.md ~/docs/book/src/user/README.md ~/docs/book/src/software/*.md ~/docs/book/src/software/*/*.md ~/docs/book/src/proposals/*/*.md
perl -pi -e "s/${OLD_MAJOR}\.${OLD_MINOR}\.${OLD_PATCH}/${NEW_MAJOR}\.${NEW_MINOR}\.${NEW_PATCH}/g" README.md ~/docs/book/src/user/README.md ~/docs/book/src/software/*.md ~/docs/book/src/software/*/*.md ~/docs/book/src/proposals/*/*.md

echo "Don't forget to:"
echo "1) ./release/deploy_web.sh"
echo "2) aws s3 cp --recursive --exclude 'data/input/*' s3://abstreet/dev/ s3://abstreet/${NEW_MAJOR}.${NEW_MINOR}.${NEW_PATCH}"
echo "3) Update map_gui/src/tools/mod.rs"
echo "4) Push the docs repo too"
