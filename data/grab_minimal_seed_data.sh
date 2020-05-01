#!/bin/bash
# Run from the base repo directory: ./data/grab_small_seed_data.sh

set -e

curl -L -o release_data.zip https://www.dropbox.com/s/wzw7gucvccoxy66/release_data.zip?dl=0
rm -rf data/system
unzip release_data.zip
mv release_data/* data
rmdir release_data
rm -f release_data.zip
