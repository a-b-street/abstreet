# Importing a new city into A/B Street

My current priority is to make Seattle work very well, but if you want to try
out A/B Street in another place, you can follow this guide. Add to
[this issue](https://github.com/dabreegster/abstreet/issues/27) if you find a
new problem.

First obtain a `.osm` with your desired area. You can use a tool like Osmosis to
clip a specific area from a large file. Put the `.osm` in `data/input/osm`.

Then you'll run some tools to import the map. Make sure you can compile
everything [from source](INSTRUCTIONS.md).

```
cd convert_osm
cargo run --release -- \
  --osm=../data/input/osm/your_city.osm \
  --drive_on_right=true|false \
  --output=../data/input/raw_maps/your_city.bin
cd ../precompute
cargo run --release -- ../data/input/raw_maps/your_city.bin
```

You should now be able to load the map using the option from the main game menu,
or by running `cd game; cargo run --release ../data/system/maps/your_city.bin`.

## Future work

There are Seattleisms baked into the code. (As of March 2020, this is out of
date; I'm actively fixing most of these.)

- `import.sh` should be generalized.
- The driving side of the road is hard-coded to the right. Look for "driving on
  the left" in `map_model/src/make/half_map.rs`.
- On-street parking is mostly not mapped in Seattle. Ideally you should fill out
  https://wiki.openstreetmap.org/wiki/Key:parking:lane for your city. I'm
  inferring these tags for most roads based on a King County GIS-specific
  dataset.
- Demand data to generate a realistic set of trips comes from an agency specific
  to the Puget Sound, but again, importing this from other sources isn't hard.
