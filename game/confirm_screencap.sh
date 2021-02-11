#!/bin/bash

city=$1;
map=$2;

rm -rf ../data/input/${city}/screenshots/${map}.zip diff screens_before;
cd screenshots/${city}/${map};
zip ${map}.zip *;
mkdir -p ../../../../data/input/${city}/screenshots/;
mv ${map}.zip ../../../../data/input/${city}/screenshots/;
cd ../../../;
rm -rf screenshots/${city}/${map};
