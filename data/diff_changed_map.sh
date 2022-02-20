#!/bin/bash
# If `updater --dry` says a mapchanged, call this to launch a UI and compare.
# This only works if you have a copy of the S3 directory in ~/s3_abst_data.

set -e

FILE=$1
if [ "$FILE" == "" ]; then
	echo Missing args;
	exit 1;
fi

rm -f old.bin
cp ~/s3_abst_data/dev/${FILE}.gz old.bin.gz
gunzip old.bin.gz
./target/release/game --dev $FILE --diff old.bin
