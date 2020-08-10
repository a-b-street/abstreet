#!/bin/bash

set -e

if [ "$1" != "" ]; then
	~/npm/node_modules/prettier/bin-prettier.js --write --prose-wrap=always $1;
	exit
fi
