# Importing a new city into A/B Street

This process isn't easy yet. Please email <dabreegster@gmail.com> or
[file a Github issue](https://github.com/dabreegster/abstreet/issues/) if you
hit problems. I'd really appreciate help and PRs to improve this.

## Quick start

Use this if you want to import a city on your computer without making it
available to other users yet.

- If you're using the **binary release** and have a `.osm` file, just do:
  `./importer --oneshot=map.osm`.

- If you're building **from source**, do: `./import.sh --oneshot=map.osm`. If
  you can't run `import.sh`, make sure you have all
  [dependencies](../dev/index.md#building-map-data). If you're using Windows and
  the console logs appear in a new window, try running the command from
  `import.sh` directly, changing the `$@` at the end to `--oneshot=map.osm` or
  whatever arguments you're passing in.

The oneshot importer will will generate a new file in `data/system/oneshot/maps`
that you can then load in the game. If you have an Osmosis polygon filter (see
below), you can also pass `--oneshot_clip=clip.poly` to improve the result. You
should first make sure your .osm has been clipped:
`osmconvert large_map.osm -B=clipping.poly --complete-ways -o=smaller_map.osm`.

By default, driving on the right is assumed. Use `--oneshot_drive_on_left` to
invert.

### How to get .osm files

If the area is small enough, try the "export" tool on
<https://www.openstreetmap.org>. You can download larger areas from
<https://download.bbbike.org/> or <http://download.geofabrik.de/index.html>,
then clip them to a smaller area. Use [geojson.io](http://geojson.io/) or
[geoman.io](https://geoman.io/geojson-editor) to draw a boundary around the
region you want to simulate and save the geojson locally. Use
`cargo run --bin geojson_to_osmosis < boundary.geojson > clipping.poly` to
convert that geojson to the
[Osmosis format](https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format)
required by osmconvert.

Note that you may hit problems if you use JOSM to download additional data to a
.osm file. Unless it updates the `<bounds/>` element, A/B Street will clip out
anything extra. The best approach is to explicitly specify the boundary with
`--oneshot_clip`.

## Including the city to A/B street more permanently

Follow this guide to add a new city to A/B street by default so other users can
use it as well.

1.  Make sure you can run `import.sh` -- see
    [the instructions](../dev/index.md#building-map-data). You'll need Rust,
    osmconvert, gdal, etc.

2.  Create a new directory: `mkdir -p data/input/your_city/polygons`

3.  Use [geojson.io](http://geojson.io/) or
    [geoman.io](https://geoman.io/geojson-editor) to draw a boundary around the
    region you want to simulate and save the geojson locally.

4.  Use `cargo run --bin geojson_to_osmosis < boundary.geojson > clipping.poly`
    to convert that geojson to the
    [Osmosis format](https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format)
    required by osmconvert.

5.  Create a new module in `importer/src/` for your city, copying
    `importer/src/krakow.rs` as a guide. Edit that file in the obvious way. The
    main thing you'll need is a .osm or .osm.pbf file to download that contains
    your city. The clipping polygon will be applied to that.

6.  Update `importer/src/main.rs` to reference your new module, following
    `krakow` as an example.

7.  Update `map_belongs_to_city` in `updater/src/main.rs`

8.  Run it: `./import.sh --city=your_city --raw --map`

9.  Update `.gitignore`, following `krakow` as an example.

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

- <https://learnosm.org>
- <https://wiki.openstreetmap.org/wiki/StreetComplete>
- [Mapping parking](map_parking.md)
