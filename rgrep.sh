#!/bin/bash

grep -IR --exclude-dir=.git --exclude-dir=target --exclude-dir=data --exclude-dir=pkg --exclude=Cargo.lock --exclude-dir=node_modules --color=auto "$@"
