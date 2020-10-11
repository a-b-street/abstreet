# Importing

This chapter describes the process of transforming OSM extracts into A/B
Street's map model. The steps are:

1.  A large .osm file is clipped to a hand-drawn boundary region, using
    `osmconvert`
2.  The `convert_osm` crate reads the clipped `.osm`, and a bunch of optional
    supplementary files, and produces a `RawMap`
3.  Part of the `map_model` crate transforms the `RawMap` into the final `Map`
4.  Other applications read and use the `Map` file

The `importer` crate orchestrates these steps, along with automatically
downloading any missing input data.

The rest of these sections describe each step in a bit more detail. Keeping the
docs up-to-date is hard; the best reference is the code, which is hopefully
organized clearly.

Don't be afraid of how complicated this pipeline seems -- each step is
relatively simple. If it helps, imagine how this started -- just chop up OSM
ways into road segments, infer lanes for each road, and infer turns between the
lanes.
