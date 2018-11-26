#!/bin/bash

release_mode=""

filter=""
test_names=""
keep_output=""
clickable_links="--clickable_links"

for arg in "$@"; do
	if [ "$arg" == "--release" ]; then
		release_mode="--release";
	elif [ "$arg" == "--fast" ]; then
		filter="--filter=Fast";
	elif [ "$arg" == "--slow" ]; then
		filter="--filter=Slow";
	elif [ "$arg" == "--keep_output" ]; then
		filter="--keep_output";
	elif [ "$arg" == "--noclickable_links" ]; then
		clickable_links="";
	elif [ "${arg:0:2}" == "--" ]; then
		echo "Unknown argument $arg";
		exit 1;
	else
		test_names="--test_names=$arg";
	fi
done

cd tests;
RUST_BACKTRACE=1 cargo run $release_mode -- $filter $keep_output $clickable_links $test_names
