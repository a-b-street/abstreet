#!/bin/bash

for x in `find */src | grep '.rs$' | grep -v pb.rs | xargs`; do
	~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustfmt $x;
done
rm */src/*.bk -f;
rm */src/*/*.bk -f;
