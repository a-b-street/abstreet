#!/bin/bash

name=$1;

rm -rf ../data/input/screenshots/${name} diff;
mv ../data/input/screenshots/pending_${name} ../data/input/screenshots/${name};
