# Testing strategy

## Unit tests

As you've probably noticed, there aren't many. Lots of the interesting behavior
in A/B Street - UI interactions, details of the simulation, map importing --
would take lots of infrastructure to specify a setup and expected outcomes. If
you have ideas for new tests, contributions always welcome! In the meantime, one
useful test covers how
[OSM tags translate into individual lanes](https://github.com/dabreegster/abstreet/blob/master/map_model/src/make/initial/lane_specs.rs).

## Screenshot diffs

Downloading fresh OSM data or modifying any part of the map importing pipeline
could easily break things. Expressing invariants about the map output is hard,
because importing is far from perfect, and OSM data is often quite buggy. So the
approach to preventing regressions here is to look for visual changes to the
final rendered map.

1.  When a new map is opted into this type of test, somebody manually squints
    carefully at it and sanity checks that it works to some degree.
2.  They use the screen capture tool in debug mode to tile the map into 1920x960
    chunks and screengrab everything.
3.  Later, somebody regenerates the map with some possible changes.
4.  They grab screenshots again, then use `compare_screenshots.sh` to quickly
    look at the visual diff. Changes to intersection geometry, number of lanes,
    rendering, etc are all easy to spot.
5.  If this manual inspection of the diff is good, they commit the new
    screenshots as the new goldenfiles.

## data/regen.sh

This tool regenerates all maps and scenarios from scratch.
`cargo run --bin updater -- --dry` then reveals what files have changed.

Additionally, this script does a few more tests:

- `--prebake` runs the full weekday scenario on two maps that've previously been
  coerced into being gridlock-free

## Integration tests

The `tests` crate contains some integration tests.

One part runs the full importer against really simple `.osm` files. To iterate
rapidly on interpreting turn restrictions, it produces goldenfiles describing
all turns in the tiny map.

The "smoke-test" section simulates one hour on all maps, flushing out bugs with
bus spawning, agents hitting odd parts of the map, etc

The "check proposals" section makes sure the edits shipped with the game still
load properly.

## Old tests

Once upon a time, I made a little test harness that would run the simulation
headlessly (without graphics), set up certain situations forcing a car to park
in a certain spot, and asserted that different `sim/src/events.rs` were produced
in the right order. The `map_editor` tool was used to manually draw really
simple maps for these situations. I deleted everything, because the effort to
specify the input and expected output were too tedious to maintain, and this
never really helped catch bugs. There was a way to label roads and buildings in
the synthetic maps, so the test code could assert person 2 made it to the
"house" building, but even with all of this, it was pretty hard.

This approach is maybe worth reviving, though.
