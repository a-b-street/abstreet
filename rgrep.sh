#!/bin/bash

grep -R --exclude-dir=.git --exclude-dir=target --exclude-dir=data --exclude-dir=in_progress --exclude=Cargo.lock --color=auto "$@"
