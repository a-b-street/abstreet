#!/bin/bash

release_mode=""

filter=""
test_names=""
keep_output=""

for arg in "$@"; do
	if [ "$arg" == "--release" ]; then
		release_mode="--release";
	elif [ "$arg" == "--fast" ]; then
		filter="--filter=Fast";
	elif [ "$arg" == "--slow" ]; then
		filter="--filter=Slow";
	elif [ "$arg" == "--keep_output" ]; then
		filter="--keep_output";
	else
		test_names="--test_names=$arg";
	fi
done

cd tests;
RUST_BACKTRACE=1 cargo run $release_mode -- $filter $keep_output $test_names
