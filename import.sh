#!/bin/bash

# If true, display importer progress from the Timer (dumped to STDOUT) in one
# pane, and all logs (on STDERR) in another. This is disabled by default until
# some issues with multitail are fixed:
#
# 1) Automatically exit when the importer is done
# 2) Stop having part of the logs blink
# 3) Get the \r / clear-line thing working for the Timer
USE_MULTITAIL=false

SPEED=--release
if [ "$1" == "--dev" ]; then
	shift
	SPEED=
fi

if ! command -v multitail &> /dev/null; then
	USE_MULTITAIL=false
fi

if $USE_MULTITAIL; then
	RUST_BACKTRACE=1 RUST_LOG_STYLE=always cargo run --bin cli $SPEED --features importer/scenarios -- import -- $@ > /tmp/abst_stdout 2> /tmp/abst_stderr &
	multitail /tmp/abst_stdout -cT ANSI /tmp/abst_stderr
else
	RUST_BACKTRACE=1 cargo run --bin cli $SPEED --features importer/scenarios -- import -- $@
fi
