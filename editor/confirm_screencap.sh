#!/bin/bash

name=$1;

rm -rf ../data/screenshots/${name};
mv ../data/screenshots/pending_${name} ../data/screenshots/${name};
