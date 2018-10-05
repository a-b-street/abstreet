#!/bin/bash

grep -R --exclude-dir=.git --exclude-dir=target --exclude-dir=data --exclude=Cargo.lock --color=always "$@"
