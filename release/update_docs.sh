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

perl -pi -e "s/${MAJOR}_${MINOR}_${OLD_PATCH}/${MAJOR}_${MINOR}_${NEW_PATCH}/g" README.md book/src/howto/README.md book/src/side_projects/santa.md
perl -pi -e "s/${MAJOR}\.${MINOR}\.${OLD_PATCH}/${MAJOR}\.${MINOR}\.${NEW_PATCH}/g" README.md book/src/howto/README.md book/src/side_projects/santa.md

echo "Don't forget to:"
echo "1) aws s3 cp --recursive s3://abstreet/dev/data/system s3://abstreet/${MAJOR}.${MINOR}.${NEW_PATCH}/data/system"
echo "2) ./release/deploy_web.sh"
echo "3) Post to r/abstreet"
echo "4) Update map_gui/src/tools/updater.rs"
