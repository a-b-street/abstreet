# Importing a new city into A/B Street

My current priority is to make Seattle work very well, but if you want to try
out A/B Street in another place, you can follow this guide. Please file a Github
issue or email <dabreegster@gmail.com> if you hit any problems.

First obtain a `.osm` with your desired area. You can use a tool like Osmosis to
clip a specific area from a large file. Put the `.osm` in `data/input/`.

Then you'll run some tools to import the map. Make sure you can compile
everything [from source](INSTRUCTIONS.md).

```
cd convert_osm
cargo run --release -- \
  --osm=../data/input/your_city.osm \
  --output=../data/raw_maps/your_city.bin
cd ../precompute
cargo run --release -- ../data/raw_maps/your_city.bin
```

You should now be able to load the map using the option from the main game menu,
or by running `cd game; cargo run --release ../data/maps/your_city.bin`.

## Future work

There are Seattleisms baked into the code.

- `import.sh` should be generalized.
- The driving side of the road is hard-coded to the right. Look for "driving on
  the left" in `map_model/src/make/half_map.rs`.
- On-street parking is inferred from a dataset specific to King County GIS. If
  your city has this information elsewhere, it should be easy to import.
- Demand data to generate a realistic set of trips comes from an agency specific
  to the Puget Sound, but again, importing this from other sources isn't hard.
