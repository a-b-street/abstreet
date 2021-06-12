#!/bin/bash
# Manually run after pushing the Github release

set -e

MAJOR=0
MINOR=2
OLD_PATCH=$1
NEW_PATCH=$2
if [ "$OLD_PATCH" == "" ] || [ "$NEW_PATCH" == "" ]; then
	echo Missing args;
	exit 1;
fi

# This assumes https://github.com/a-b-street/docs is checked out at ~/docs
perl -pi -e "s/${MAJOR}_${MINOR}_${OLD_PATCH}/${MAJOR}_${MINOR}_${NEW_PATCH}/g" README.md ~/docs/book/src/user/README.md ~/docs/book/src/software/*.md
perl -pi -e "s/${MAJOR}\.${MINOR}\.${OLD_PATCH}/${MAJOR}\.${MINOR}\.${NEW_PATCH}/g" README.md ~/docs/book/src/user/README.md ~/docs/book/src/software/*.md

echo "Don't forget to:"
echo "1) ./release/deploy_web.sh"
echo "2) aws s3 cp --recursive --exclude 'data/input/*' s3://abstreet/dev/ s3://abstreet/${MAJOR}.${MINOR}.${NEW_PATCH}"
echo "3) Post to r/abstreet"
echo "4) Update map_gui/src/tools/updater.rs"
echo "5) Push the docs repo too"
