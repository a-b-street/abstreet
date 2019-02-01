#!/bin/bash

grep -IR --exclude-dir=.git --exclude-dir=target --exclude-dir=data --exclude-dir=initial_maps --exclude=Cargo.lock --color=auto "$@"
