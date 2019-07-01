#!/bin/bash

name=$1;

rm -rf ../data/screenshots/${name} diff;
mv ../data/screenshots/pending_${name} ../data/screenshots/${name};
