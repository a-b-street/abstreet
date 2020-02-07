#!/bin/bash
# Run from the base repo directory: ./data/grab_all_seed_data.sh

set -e

curl -L -o seed_data.zip https://www.dropbox.com/s/3zkf5w6zhwvbif5/seed_data.zip?dl=0
rm -rf data/input data/system
unzip seed_data.zip
rm -f seed_data.zip
# The zip clobbers the .gitignore symlinks; just let git take care of it
git checkout data/system
