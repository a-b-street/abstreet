#!/bin/bash

grep -IR --exclude-dir=.git --exclude-dir=target --exclude-dir=data --exclude-dir=book --exclude=Cargo.lock --color=auto "$@"
