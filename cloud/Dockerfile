FROM ubuntu:20.04

WORKDIR /abstreet
COPY target/release/importer ./target/release/
COPY target/release/updater ./target/release/
COPY data/MANIFEST.json ./data/
COPY importer/config ./importer/config/
COPY cloud/import_one_city.sh .
