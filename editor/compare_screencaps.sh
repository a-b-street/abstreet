#!/bin/bash

name=$1;
before=../data/screenshots/pending_$name;
after=../data/screenshots/$name;

rm -rf diff
mkdir diff

for file in `ls $before | grep -v full.png | grep -v combine.sh | grep -v MANIFEST`; do
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
