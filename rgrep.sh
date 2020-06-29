#!/bin/bash

grep -IR --exclude-dir=.git --exclude-dir=target --exclude-dir=data --exclude=Cargo.lock --color=auto "$@"
