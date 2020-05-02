# Importing a new city into A/B Street

This process isn't easy yet. Please email <dabreegster@gmail.com> or
[file a Github issue](https://github.com/dabreegster/abstreet/issues/) if you
hit problems. I'd really appreciate help and PRs to improve this.

## Quick start

TODO. Ideally, you just run the importer with a local .osm file and an optional
clipping polygon. No code changes.

## Including the city by default

1.  Make sure you can run `import.sh` -- see
    [the instructions](dev.md#building-map-data). You'll need Rust, osmconvert,
    gdal, etc.

2.  Use http://geojson.io/ to draw a polygon around the region you want to
    simulate.

3.  Create a new directory: `mkdir -p data/input/your_city/polygons`

4.  Create a
    [polygon filter file](https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format)
    in that directory using the coordinates from geojson.io. It's easiest to
    start with an existing file from another directory; I recommend
    `data/input/austin/polygons/downtown_atx.poly` as a guide.

5.  Create a new module in `importer/src/` for your city, copying
    `importer/src/austin.rs` as a guide. Edit that file in the obvious way. The
    main thing you'll need is a .osm or .osm.pbf file to download that contains
    your city. The clipping polygon will be applied to that.

6.  Update `importer/src/main.rs` to reference your new module, following
    `austin` as an example.

7.  Update `map_belongs_to_city` in `updater/src/main.rs`

8.  Run it: `./import.sh --city=your_city --raw --map`

9.  Update `.gitignore`, following `austin` as an example.

Send a PR with your changes! I'll generate everything and make it work with
`updater`, so most people don't have to build everything from scratch.

## Next steps

OpenStreetMap isn't the only data source we need. If you look at the import
pipeline for Seattle, you'll see many more sources for parking, GTFS bus
schedules, person/trip demand data for scenarios, etc. Most of these aren't
standard between cities. If you want to make your city more realistic, we'll
have to import more data. Get in touch.
