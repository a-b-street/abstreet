#!/bin/bash
# Used to figure out when it's time to run package_for_devs.sh

set -e

find data/input data/system/ -type f \( -not -name "MANIFEST.txt" \) -exec md5sum '{}' \; > data/MANIFEST.txt
