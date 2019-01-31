#!/bin/bash

grep -R --exclude-dir=.git --exclude-dir=target --exclude-dir=data --exclude-dir=initial_maps --exclude=Cargo.lock --color=auto "$@"
