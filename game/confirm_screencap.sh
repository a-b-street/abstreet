#!/bin/bash

country=$1
city=$2;
map=$3;

rm -rf ../data/input/${country}/${city}/screenshots/${map}.zip diff screens_before;
cd screenshots/${country}/${city}/${map};
zip ${map}.zip *;
mkdir -p ../../../../data/input/${country}/${city}/screenshots/;
mv ${map}.zip ../../../../data/input/${country}/${city}/screenshots/;
cd ../../../;
rm -rf screenshots/${country}/${city}/${map};
