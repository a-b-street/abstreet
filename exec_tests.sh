#!/bin/bash

release_mode=""
filter=""

for arg in "$@"; do
	if [ "$arg" == "--release" ]; then
		release_mode="--release";
	elif [ "$arg" == "--fast" ]; then
		filter="--filter=Fast";
	elif [ "$arg" == "--slow" ]; then
		filter="--filter=Slow";
	fi
done

cd tests;
RUST_BACKTRACE=1 cargo run $release_mode -- $filter
