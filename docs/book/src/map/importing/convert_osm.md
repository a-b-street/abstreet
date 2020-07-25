# From OSM to RawMap (`convert_osm` crate)

The first phase of map building reads in data from OSM files and a few others,
producing a serialized `RawMap`. Importing all maps (one for each pre-defined
bounding polygon) takes a few minutes. Players don't see this cost; it only
takes a few seconds to load a serialized map.

- `osm.rs`: Read .osm, extracting the points for road-like ways, buildings, and
  areas
  - Areas usually come from a relation of multiple ways, with the points out of
    order. Gluing all the points together fails when the .osm has some ways
    clipped out. In that case, try to trace along the map boundary if the
    partial area intersects the boundary in a clear way. Otherwise, just use a
    straight line to try to close off the polygon.
  - Also read traffic signal locations and turn restrictions between OSM ways
- `split_ways.rs`: Split OSM ways into road segments
  - OSM ways cross many intersections, so treat points with multiple ways and
    the points at the beginning and end of a way as intersections, then split
    the way into road segments between two intersections.
  - This phase remembers which road segment is the beginning and end of the OSM
    way, for per-lane turn restrictions later
  - Apply turn restrictions between roads here. Since OSM ways cross many
    intersections, the turn restrictions only apply to one particular road
    segment that gets created from the way. Make sure the destination of the
    restriction is actually incident to a particular source road.
- `clip.rs`: Clip the map to the boundary polygon
  - Osmosis options in `import.sh` preserve ways that cross the boundary
  - Trim roads that cross the boundary. There may be cases where a road dips out
    of bounds, then immediately comes back in. Disconnecting it isn't ideal, but
    it's better to manually tune the boundary polygon when this happens than try
    to preserve lots of out-of-bounds geometry.
  - Area polygons are intersected with the boundary polygon using the `clipping`
    crate
- `lib.rs`: Remove cul-de-sacs (roads that begin and end at the same
  intersection), because they mess up parking hints and pathfinding.
- `lib.rs`: Apply parking hints from a King County GIS blockface dataset
  - Match each blockface to the nearest edge of a road
  - Interpret the metadata to assign on-street parking there or not
- `lib.rs`: Apply offstreet parking hints from a King County GIS dataset
  - Match each point to the building containing it, plumbing through the number
    of spots
- `lib.rs`: **Disabled**: Apply sidewalk presence hints from a King County GIS
  dataset
  - Match each sidewalk line to the nearest edge of a road
  - Update the road to have a sidewalk on none, one, or both sides
- `lib.rs` using the `srtm` module: Load (extremely poor quality) elevation data
