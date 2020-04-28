#!/bin/bash

name=$1;
before=../data/input/screenshots/$name;
after=../data/input/screenshots/pending_$name;

rm -rf diff
mkdir diff

for file in `ls $before | grep -v full.png | grep -v combine.sh`; do
	# For whatever reason, the intersection annotation doesn't seem to
	# always match up between two captures.
	prefix=`echo $file | sed 's/_.*//' | sed 's/.png//'`;

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
