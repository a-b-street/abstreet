# Importing a new city into A/B Street

This process isn't easy yet. Please email <dabreegster@gmail.com> or
[file a Github issue](https://github.com/dabreegster/abstreet/issues/) if you
hit problems. I'd really appreciate help and PRs to improve this.

## Quick start

If you have a `.osm` file, you can just run
`./import.sh --oneshot=/absolute/path/to/map.osm`. This tool will generate a new
file in `data/system/maps` that you can then load in the game.

If you're using a binary release, you have to be sure to run the tool from the
`importer/` directory, so that `../data/` exists:
`cd importer; ./importer --oneshot=/absolute/path/to/file.osm`

If you have an Osmosis polygon filter (see below), you can also pass
`--oneshot_clip=/absolute/path/to/clip.poly` to improve the result. You should
first make sure your .osm has been clipped:
`osmconvert large_map.osm -B=clipping.poly --complete-ways -o=smaller_map.osm`.

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
