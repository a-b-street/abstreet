#!/bin/bash
# TODO describe

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
