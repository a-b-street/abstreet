#!/bin/bash

before=$1;
after=$2;

rm -rf diff
mkdir diff

for file in `ls $before | grep -v full.png`; do
	diff $before/$file $after/$file;
	if [ $? -eq 1 ]; then
		compare $before/$file $after/$file diff/$file;
		if [ "$3" == "-i" ]; then
			feh diff/$file $before/$file $after/$file;
			# Handle interrupts by killing the script entirely
			if [ $? -ne 0 ]; then
				exit;
			fi
		fi
	fi
done
