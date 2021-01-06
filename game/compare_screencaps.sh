#!/bin/bash

city=$1;
map=$2;

mkdir screens_before;
cd screens_before;
unzip ../../data/input/${city}/screenshots/${map}.zip;
cd ..;
before=screens_before;
after=screenshots/${city}/${map};

rm -rf diff
mkdir diff

for file in `ls $before | grep -v full.png | grep -v combine.sh`; do
	# For whatever reason, the intersection annotation doesn't seem to
	# always match up between two captures.
	prefix=`echo $file | sed 's/_.*//' | sed 's/.png//' | sed 's/.gif//'`;

	diff $before/${prefix}* $after/${prefix}*;
	if [ $? -eq 1 ]; then
		compare $before/${prefix}* $after/${prefix}* diff/${prefix}.png;
		feh diff/${prefix}.png $before/${prefix}* $after/${prefix}*;
		# Handle interrupts by killing the script entirely
		if [ $? -ne 0 ]; then
			exit;
		fi
	fi
done
