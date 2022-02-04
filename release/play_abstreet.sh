#!/bin/bash

echo See logs in output.txt
RUST_BACKTRACE=1 ./game 1> output.txt 2>&1
