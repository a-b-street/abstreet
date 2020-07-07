#!/bin/bash

name=$1;

rm -rf ../data/input/screenshots/${name}.zip diff screens_before;
zip -r $name screenshots_${name};
mv ${name}.zip ../data/input/screenshots/;
rm -rf screenshots_${name};
