# Road/intersection geometry: RawMap to InitialMap

The remainder of map construction is done in the `map_model` crate. There's one
intermediate structure between `RawMap` and `Map`, called `InitialMap`.

- `make/remove_disconnected.rs`: Remove disconnected roads
  - Just floodfill from some road, assuming all roads are bidirectional, to get
    different partitions.
  - Remove roads from all but the largest partition
- `make/initial/mod.rs` and `make/initial/lane_specs.rs`: Interpret OSM tags to
  figure out what lanes are on each side of each road, also figuring out the
  total width of the road.
- `make/initial/geometry.rs`: Figure out the polygon for each intersection, and
  trim back road center-lines to end at a face of the polygon.
  - For every road touching the intersection, get the polyline of each side,
    based on the road's width
    - See appendix for how to shift polylines
  - Sort all the polylines by the angle to the intersection's shared point
  - Intersect every polyline with every other polyline
    - More specifically -- the second half of each polyline, to get the correct
      collision point
    - Look at the perpendicular infinite line to the collision point on the
      shifted polyline, then find where it hits the original center line. Trim
      back the center line by the max distance from these collisions.
  - Compute the intersection's polygon by considering collisions between
    adjacent roads' polylines
  - Deal with short roads and floating point issues by deduping any adjacent
    points closer than 0.1m
