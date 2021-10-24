#!/bin/bash
# If `updater --dry` says a file changed, like
# `data/system/us/seattle/maps/montlake.bin`, call this to dump the old and new
# versions of the file to JSON and compare them manually. This only works if
# you have a copy of the S3 directory in ~/s3_abst_data.

set -e

FILE=$1
if [ "$FILE" == "" ]; then
	echo Missing args;
	exit 1;
fi

cp ~/s3_abst_data/dev/${FILE}.gz old.bin.gz
gunzip old.bin.gz
./target/release/cli dump-json old.bin > old.json
rm -f old.bin

./target/release/cli dump-json $FILE > new.json

echo "diff old.json new.json"
echo "mold old.json new.json   # slower"
