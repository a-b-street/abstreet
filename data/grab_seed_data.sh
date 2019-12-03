#!/bin/bash
# Run from the base repo directory: ./data/grab_seed_data.sh

set -e

curl -L -o seed_data.zip http://dropbox.com/TODO	# TODO Need to upload this
rm -rf data/input data/system
unzip seed_data.zip
rm -f seed_data.zip
