# Importing a new city into A/B Street

My current priority is to make Seattle work very well, but if you want to try
out A/B Street in another place, you can follow this guide. Please file a Github
issue or email <dabreegster@gmail.com> if you hit any problems.

First you need to prepare input. Obtain a `.osm` file and create an
[Osmosis polygon filter file](https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format)
for the area you want to import. You can clip a `.osm` like this:

```
osmosis \
  --read-xml enableDateParsing=no file=large_area.osm \
  --bounding-polygon file=clip.poly completeWays=true \
  --write-xml data/input/your_city.osm
```

Then you'll run some tools to import the map. Make sure you can compile
everything [from source](INSTRUCTIONS.md). Keep `clip.poly` around for the next
command.

```
cd convert_osm
cargo run --release -- \
  --osm=../data/input/your_city.osm \
  --clip=../data/input/clip.poly \
  --output=../data/raw_maps/your_city.bin
cd ../precompute
cargo run --release -- ../data/raw_maps/your_city.bin
```

You should now be able to load the map using the option from the main game menu,
or by running `cd editor; cargo run --release ../data/maps/your_city.bin`.

## Future work

There are Seattleisms baked into the code.

- `import.sh` should be generalized.
- The driving side of the road is hard-coded to the right. Look for "driving on
  the left" in `map_model/src/make/half_map.rs`.
