# Importing a new city into A/B Street

My current priority is to make Seattle work very well, but if you want to try
out A/B Street in another place, you can follow this guide. Please file a Github
issue or email <dabreegster@gmail.com> if you hit any problems.

First make sure you can compile everything [from source](INSTRUCTIONS.md). Put
some `.osm` (pre-clipped to whatever area you want via Osmosis or something
else) into `data/input`. Then run:

```
cd convert_osm
cargo run --release -- \
  --osm=../data/input/your_city.osm \
  --output=../data/raw_maps/your_city.bin
cd ../precompute
cargo run --release -- ../data/raw_maps/your_city.bin
```

You should now be able to load the map using the option from the main game menu,
or by running `cd editor; cargo run --release ../data/maps/your_city.osm`.

## Future work

There are Seattleisms baked into the code.

- `import.sh` should be generalized.
- The driving side of the road is hard-coded to the right. Look for "driving on
  the left" in `map_model/src/make/half_map.rs`.
