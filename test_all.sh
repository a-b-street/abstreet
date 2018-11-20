#!/bin/bash

if [ "$1" = "--fast" ]; then
	cargo test --no-fail-fast
else
	cargo test --release --no-fail-fast
fi
