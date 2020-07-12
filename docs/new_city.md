# Importing a new city into A/B Street

This process isn't easy yet. Please email <dabreegster@gmail.com> or
[file a Github issue](https://github.com/dabreegster/abstreet/issues/) if you
hit problems. I'd really appreciate help and PRs to improve this.

## Quick start

If you're using the binary release and have a `.osm` file, just do:
`./importer/importer --oneshot=map.osm`.

If you're building from source, do: `./import.sh --oneshot=map.osm`. If you
can't run `import.sh`, make sure you have all
[dependencies](dev.md#building-map-data).

The oneshot importer will will generate a new file in `data/system/maps` that
you can then load in the game. If you have an Osmosis polygon filter (see
below), you can also pass `--oneshot_clip=clip.poly` to improve the result. You
should first make sure your .osm has been clipped:
`osmconvert large_map.osm -B=clipping.poly --complete-ways -o=smaller_map.osm`.

You can also try `--oneshot_drive_on_left`, but you'll spot some bugs. Get in
touch if you need these fixed soon or want to help.

## How to get .osm files

If the area is small enough, try the "export" tool on
https://www.openstreetmap.org. You can download larger areas from
https://download.bbbike.org/ or http://download.geofabrik.de/index.html, then
clip them to a smaller area. You can draw a clipping polygon using
http://geojson.io or https://geoman.io/geojson-editor. You have to turn the
coordinates from that polygon into the
[Osmosis format](https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format).
The `data/geojson_to_osmosis.py` script can help with the formatting.

## Including the city by default

1.  Make sure you can run `import.sh` -- see
    [the instructions](dev.md#building-map-data). You'll need Rust, osmconvert,
    gdal, etc.

2.  Use [geojson.io](http://geojson.io/) or
    [geoman.io](https://geoman.io/geojson-editor) to draw a polygon around the
    region you want to simulate.

3.  Create a new directory: `mkdir -p data/input/your_city/polygons`

4.  Create a
    [polygon filter file](https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format)
    in that directory using the coordinates from geojson.io. It's easiest to
    start with an existing file from another directory; I recommend
    `data/input/austin/polygons/downtown_atx.poly` as a guide. You can use
    `data/geojson_to_osmosis.py` to help format the coordinates.

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

You may notice issues with OSM data while using A/B Street. Some of these are
bugs in A/B Street itself, but others are incorrectly tagged lanes. Some
resources for fixing OSM:

- https://learnosm.org
- https://wiki.openstreetmap.org/wiki/StreetComplete
- https://dabreegster.github.io/abstreet/map_parking.html
